from django.shortcuts import render, redirect
from django.http import HttpResponse
import os
import json
import logging
import shutil

logger = logging.getLogger(__name__)

from .services import fetch_available_models
from .utils import (
    load_config, save_config, group_models_by_provider,
    flatten_models_with_provider_prefix, ensure_sessions_dir
)

def index(request):
    """Main entry point - redirects to chat or setup"""
    config = load_config()
    if not config or not config.get("OPENROUTER_API_KEY"):
        return redirect('setup')
    return redirect('chat')

def setup_wizard(request):
    """First-time setup wizard - 2 steps: API key validation, model selection"""
    # Check if already configured (both API key AND model must be set)
    config = load_config()
    if config and config.get("OPENROUTER_API_KEY") and config.get("MODEL"):
        return redirect('index')

    # Initialize session variables
    if 'setup_step' not in request.session:
        request.session['setup_step'] = 1
        request.session.modified = True
        # Note: No need to store API key or models in session
        # API key is written to config.json in step 1

    step = request.session.get('setup_step', 1)

    # Step 1: API Key
    if step == 1:
        if request.method == 'POST':
            api_key = request.POST.get('api_key', '').strip()

            if not api_key:
                return render(request, 'setup/step1.html', {
                    'error': 'Please enter an API key'
                })

            # Validate API key by fetching models
            models = fetch_available_models(api_key)
            logger.info(f"fetch_available_models returned: {len(models) if models else 'None'} models")

            if models and len(models) > 0:
                # Write partial config.json with API key immediately
                partial_config = {
                    "OPENROUTER_API_KEY": api_key,
                    "MODEL": "",  # To be filled in step 2
                    "SITE_URL": "https://liminalsalt.app",
                    "SITE_NAME": "Liminal Salt",
                    "DEFAULT_PERSONALITY": "assistant",
                    "PERSONALITIES_DIR": "personalities",
                    "MAX_HISTORY": 50,
                    "SESSIONS_DIR": "sessions",
                    "LTM_FILE": "long_term_memory.md"
                }
                save_config(partial_config)
                logger.info("API key validated and saved to config.json")

                # Only store step in session - no API key or models
                request.session['setup_step'] = 2
                request.session.modified = True
                logger.info("Advancing to step 2")
                return redirect('setup')
            else:
                logger.error(f"API key validation failed: models={models}")
                return render(request, 'setup/step1.html', {
                    'error': 'Invalid API key or connection error. Please check the server logs for details.',
                    'api_key': api_key
                })

        return render(request, 'setup/step1.html')

    # Step 2: Model Selection
    elif step == 2:
        # Load API key from config.json (written in step 1)
        config = load_config()
        api_key = config.get('OPENROUTER_API_KEY')

        if not api_key:
            # Something went wrong, go back to step 1
            logger.error("No API key found in config.json at step 2")
            request.session['setup_step'] = 1
            request.session.modified = True
            return redirect('setup')

        if request.method == 'POST':
            selected_model = request.POST.get('model', '').strip()

            if not selected_model:
                # Re-fetch models for error display
                models = fetch_available_models(api_key)
                if models:
                    grouped_models = group_models_by_provider(models)
                    model_options = flatten_models_with_provider_prefix(grouped_models)
                    return render(request, 'setup/step2.html', {
                        'error': 'Please select a model',
                        'model_count': len(models),
                        'model_options': model_options,
                        'selected_model': selected_model
                    })
                else:
                    # API key is no longer valid, go back to step 1
                    logger.error("Failed to re-fetch models in step 2")
                    request.session['setup_step'] = 1
                    request.session.modified = True
                    return redirect('setup')

            # Update config.json with selected model (config already has API key)
            config['MODEL'] = selected_model
            save_config(config)
            logger.info(f"Setup complete: model {selected_model} saved to config.json")

            # Clean up session
            del request.session['setup_step']
            request.session.modified = True

            return redirect('chat')

        # Display step 2 form - fetch models using API key from config
        logger.info("Fetching models for step 2 display from config.json")
        models = fetch_available_models(api_key)

        if not models or len(models) == 0:
            # API key is no longer valid, go back to step 1
            logger.error("Failed to fetch models for step 2 display")
            request.session['setup_step'] = 1
            request.session.modified = True
            return redirect('setup')

        grouped_models = group_models_by_provider(models)
        model_options = flatten_models_with_provider_prefix(grouped_models)

        return render(request, 'setup/step2.html', {
            'model_count': len(models),
            'model_options': model_options
        })

def chat(request):
    """Main chat view - session determined from Django session storage"""
    from datetime import datetime
    from .services import load_context, get_available_personalities
    from .services import ChatCore
    from .services import Summarizer
    from django.conf import settings
    from .utils import (
        get_sessions_with_titles, group_sessions_by_personality,
        get_current_session, set_current_session, get_collapsed_personalities,
        title_has_artifacts, ensure_sessions_dir
    )

    ensure_sessions_dir()
    config = load_config()
    if not config or not config.get("OPENROUTER_API_KEY"):
        return redirect('setup')

    # Get session_id from Django session storage
    session_id = get_current_session(request)

    # If no current session, load first available or create new
    if not session_id:
        sessions = get_sessions_with_titles()
        if sessions:
            session_id = sessions[0]["id"]
        else:
            session_id = f"session_{datetime.now().strftime('%Y%m%d_%H%M%S')}.json"
        set_current_session(request, session_id)

    # Load session data
    session_path = settings.SESSIONS_DIR / session_id
    personalities_dir = str(settings.PERSONALITIES_DIR)
    ltm_file = str(settings.LTM_FILE)
    api_key = config.get("OPENROUTER_API_KEY")
    model = config.get("MODEL", "anthropic/claude-haiku-4.5")
    max_history = config.get("MAX_HISTORY", 50)

    # Try to load personality from session file
    session_personality = None
    if os.path.exists(session_path):
        try:
            with open(session_path, 'r') as f:
                data = json.load(f)
                if isinstance(data, dict):
                    session_personality = data.get("personality")
        except:
            pass

    # Fallback to default
    if not session_personality:
        session_personality = config.get("DEFAULT_PERSONALITY", "assistant")

    # Load context and create ChatCore
    personality_path = os.path.join(personalities_dir, session_personality)
    system_prompt = load_context(personality_path, ltm_file=ltm_file)

    chat_core = ChatCore(
        api_key=api_key,
        model=model,
        system_prompt=system_prompt,
        max_history=max_history,
        history_file=str(session_path),
        personality=session_personality
    )

    # Handle message sending
    if request.method == 'POST' and 'message' in request.POST:
        user_message = request.POST.get('message', '').strip()
        if user_message:
            response = chat_core.send_message(user_message)

            # Handle title generation (3-tier logic)
            summarizer = Summarizer(api_key, model)

            # Get first user message
            first_user_msg = ""
            for msg in chat_core.messages:
                if msg["role"] == "user":
                    first_user_msg = msg["content"]
                    break

            # TIER 1 & 2: Generate title if still "New Chat"
            if chat_core.title == "New Chat" and not response.startswith("ERROR:") and first_user_msg:
                new_title = summarizer.generate_title(first_user_msg, response)
                chat_core.title = new_title
                chat_core._save_history()

            # TIER 3: Fix malformed titles
            elif title_has_artifacts(chat_core.title) and not response.startswith("ERROR:") and first_user_msg:
                new_title = summarizer.generate_title(first_user_msg, response)
                chat_core.title = new_title
                chat_core._save_history()

            # Redirect to refresh page
            return redirect('chat')

    # Prepare sidebar data
    sessions = get_sessions_with_titles()
    grouped_sessions = group_sessions_by_personality(sessions)
    collapsed_personalities = get_collapsed_personalities(request)

    # Get available personalities for new chat modal
    available_personalities = get_available_personalities(str(settings.PERSONALITIES_DIR))
    default_personality = config.get("DEFAULT_PERSONALITY", "assistant")

    context = {
        'session_id': session_id,
        'title': chat_core.title,
        'personality': chat_core.personality,
        'model': model,
        'messages': chat_core.messages,
        'sessions': sessions,
        'grouped_sessions': grouped_sessions,
        'collapsed_personalities': collapsed_personalities,
        'current_session': session_id,
        'available_personalities': available_personalities,
        'default_personality': default_personality,
    }

    # Check if HTMX request - return partial template for sidebar session switching
    if request.headers.get('HX-Request'):
        return render(request, 'chat/chat_main.html', context)

    return render(request, 'chat/chat.html', context)


def switch_session(request):
    """HTMX endpoint to switch current session"""
    from .utils import get_current_session, set_current_session

    if request.method == 'POST':
        session_id = request.POST.get('session_id')
        if session_id:
            set_current_session(request, session_id)

    # Return the chat main partial (reuses chat view logic)
    return chat(request)


def new_chat(request):
    """Create new chat (POST)"""
    from datetime import datetime
    from .services import get_available_personalities
    from django.conf import settings

    config = load_config()
    if not config:
        return redirect('setup')

    # Use Django settings for absolute path instead of config's relative path
    available_personalities = get_available_personalities(str(settings.PERSONALITIES_DIR))
    default_personality = config.get("DEFAULT_PERSONALITY", "assistant")

    if request.method == 'POST':
        from .utils import set_current_session
        from django.urls import reverse

        selected_personality = request.POST.get('personality', default_personality)
        new_id = f"session_{datetime.now().strftime('%Y%m%d_%H%M%S')}.json"

        # Store personality in session for new chat
        if 'session_personalities' not in request.session:
            request.session['session_personalities'] = {}
        request.session['session_personalities'][new_id] = selected_personality
        request.session.modified = True

        # Set as current session
        set_current_session(request, new_id)

        # Create initial session file so it appears in sidebar immediately
        session_path = settings.SESSIONS_DIR / new_id
        initial_data = {
            "title": "New Chat",
            "personality": selected_personality,
            "messages": []
        }
        with open(session_path, 'w') as f:
            json.dump(initial_data, f)

        # For HTMX requests, use HX-Redirect header for full page reload
        if request.headers.get('HX-Request'):
            response = HttpResponse()
            response['HX-Redirect'] = reverse('chat')
            return response

        return redirect('chat')

    # GET request - redirect to chat (new chat is now a modal)
    return redirect('chat')


def delete_chat(request):
    """Delete current chat session (POST)"""
    from django.conf import settings
    from .utils import get_sessions_with_titles, set_current_session, get_current_session
    from datetime import datetime

    if request.method == 'POST':
        # Get current session from storage
        session_id = get_current_session(request)
        if not session_id:
            return redirect('chat')

        session_path = settings.SESSIONS_DIR / session_id
        if os.path.exists(session_path):
            os.remove(session_path)

        # Switch to another session
        remaining = [s for s in get_sessions_with_titles() if s["id"] != session_id]
        if remaining:
            set_current_session(request, remaining[0]["id"])
        else:
            # Create new session
            new_id = f"session_{datetime.now().strftime('%Y%m%d_%H%M%S')}.json"
            set_current_session(request, new_id)

        return redirect('chat')

    return redirect('chat')


def send_message(request):
    """Send message to chat (HTMX endpoint) - returns HTML fragment"""
    from datetime import datetime
    from .services import load_context
    from .services import ChatCore
    from .services import Summarizer
    from django.conf import settings
    from .utils import title_has_artifacts, get_current_session

    if request.method != 'POST':
        return HttpResponse(status=405)  # Method not allowed

    user_message = request.POST.get('message', '').strip()
    if not user_message:
        return HttpResponse(status=400)  # Bad request

    # Get current session from storage
    session_id = get_current_session(request)
    if not session_id:
        return HttpResponse('<div class="message error">No active session</div>')

    # Load config
    config = load_config()
    if not config or not config.get("OPENROUTER_API_KEY"):
        return HttpResponse('<div class="message error">Configuration error: API key not found</div>')

    # Load session data (same as chat view)
    session_path = settings.SESSIONS_DIR / session_id
    personalities_dir = str(settings.PERSONALITIES_DIR)
    ltm_file = str(settings.LTM_FILE)
    api_key = config.get("OPENROUTER_API_KEY")
    model = config.get("MODEL", "anthropic/claude-haiku-4.5")
    max_history = config.get("MAX_HISTORY", 50)

    # Load personality from session file
    session_personality = None
    if os.path.exists(session_path):
        try:
            with open(session_path, 'r') as f:
                data = json.load(f)
                if isinstance(data, dict):
                    session_personality = data.get("personality")
        except:
            pass

    if not session_personality:
        session_personality = config.get("DEFAULT_PERSONALITY", "assistant")

    # Load context and create ChatCore
    personality_path = os.path.join(personalities_dir, session_personality)
    system_prompt = load_context(personality_path, ltm_file=ltm_file)

    chat_core = ChatCore(
        api_key=api_key,
        model=model,
        system_prompt=system_prompt,
        max_history=max_history,
        history_file=str(session_path),
        personality=session_personality
    )

    # Send message and get response
    assistant_message = chat_core.send_message(user_message)

    # Handle title generation (same logic as chat view)
    summarizer = Summarizer(api_key, model)
    first_user_msg = ""
    for msg in chat_core.messages:
        if msg["role"] == "user":
            first_user_msg = msg["content"]
            break

    # Track if title changed
    title_changed = False
    old_title = chat_core.title

    # Generate or fix title
    if chat_core.title == "New Chat" and not assistant_message.startswith("ERROR:") and first_user_msg:
        new_title = summarizer.generate_title(first_user_msg, assistant_message)
        chat_core.title = new_title
        chat_core._save_history()
        title_changed = True
    elif title_has_artifacts(chat_core.title) and not assistant_message.startswith("ERROR:") and first_user_msg:
        new_title = summarizer.generate_title(first_user_msg, assistant_message)
        chat_core.title = new_title
        chat_core._save_history()
        title_changed = True

    # Return HTML fragment for HTMX (only assistant message, user already shown)
    response = render(request, 'chat/assistant_fragment.html', {
        'assistant_message': assistant_message
    })

    # Add headers for title update if changed
    if title_changed:
        response['X-Chat-Title'] = chat_core.title
        response['X-Chat-Session-Id'] = session_id

    return response

def memory(request):
    """User memory view"""
    from datetime import datetime
    from django.conf import settings
    from .utils import aggregate_all_sessions_messages

    config = load_config()
    if not config:
        return redirect('setup')

    ltm_file = settings.LTM_FILE
    model = config.get("MODEL", "")

    # Get last update time from file
    last_update = None
    if os.path.exists(ltm_file):
        mtime = os.path.getmtime(ltm_file)
        last_update = datetime.fromtimestamp(mtime)

    # Read memory content
    memory_content = ""
    if os.path.exists(ltm_file):
        with open(ltm_file, 'r') as f:
            memory_content = f.read()

    context = {
        'model': model,
        'memory_content': memory_content,
        'last_update': last_update,
        'success': request.GET.get('success'),
        'error': request.GET.get('error'),
    }

    # Return partial for HTMX requests, redirect others to chat
    if request.headers.get('HX-Request'):
        return render(request, 'memory/memory_main.html', context)

    return redirect('chat')


def update_memory(request):
    """Update long-term memory (POST)"""
    from django.conf import settings
    from django.urls import reverse
    from datetime import datetime
    from .services import Summarizer
    from .utils import aggregate_all_sessions_messages

    if request.method == 'POST':
        config = load_config()
        ltm_file = settings.LTM_FILE
        api_key = config.get("OPENROUTER_API_KEY")
        model = config.get("MODEL")

        success_msg = None
        error_msg = None

        try:
            # Aggregate messages from all sessions
            all_messages = aggregate_all_sessions_messages()

            if not all_messages:
                error_msg = "No messages found in any session"
            else:
                # Update memory
                summarizer = Summarizer(api_key, model)
                summarizer.update_long_term_memory(all_messages, str(ltm_file))
                success_msg = "Memory Updated"

        except Exception as e:
            error_msg = f"Memory update failed: {str(e)}"

        # For HTMX requests, return the partial directly
        if request.headers.get('HX-Request'):
            # Re-read the memory content
            memory_content = ""
            last_update = None
            if os.path.exists(ltm_file):
                with open(ltm_file, 'r') as f:
                    memory_content = f.read()
                last_update = datetime.fromtimestamp(os.path.getmtime(ltm_file))

            context = {
                'model': model,
                'memory_content': memory_content,
                'last_update': last_update,
                'success': success_msg,
                'error': error_msg,
                'just_updated': True if success_msg else False,
            }
            return render(request, 'memory/memory_main.html', context)

        # For regular requests, redirect with query params
        if error_msg:
            return redirect(f"{reverse('memory')}?error={error_msg}")
        return redirect(f"{reverse('memory')}?success={success_msg}")

    return redirect('memory')


def wipe_memory(request):
    """Wipe long-term memory (POST)"""
    from django.conf import settings
    from django.urls import reverse

    if request.method == 'POST':
        config = load_config()
        ltm_file = settings.LTM_FILE
        if os.path.exists(ltm_file):
            os.remove(ltm_file)

        # For HTMX requests, return the partial directly
        if request.headers.get('HX-Request'):
            context = {
                'model': config.get("MODEL", ""),
                'memory_content': "",
                'last_update': None,
                'success': "Memory wiped successfully",
                'error': None,
                'just_updated': True,
            }
            return render(request, 'memory/memory_main.html', context)

        return redirect(f"{reverse('memory')}?success=Memory wiped successfully")

    return redirect('memory')


def modify_memory(request):
    """Modify memory based on user command (HTMX endpoint)"""
    from django.conf import settings
    from datetime import datetime
    from .services import Summarizer

    if request.method != 'POST':
        return HttpResponse(status=405)

    command = request.POST.get('command', '').strip()
    if not command:
        return HttpResponse(status=400)

    config = load_config()
    if not config:
        return HttpResponse("Configuration not found", status=500)

    api_key = config.get("OPENROUTER_API_KEY")
    model = config.get("MODEL")
    ltm_file = settings.LTM_FILE

    # Call the summarizer to modify memory
    summarizer = Summarizer(api_key, model)
    updated_memory = summarizer.modify_memory_with_command(command, str(ltm_file))

    # Get last update time
    last_update = None
    if os.path.exists(ltm_file):
        last_update = datetime.fromtimestamp(os.path.getmtime(ltm_file))

    # Return the updated memory view
    context = {
        'model': model,
        'memory_content': updated_memory if updated_memory else "",
        'last_update': last_update,
        'success': "Memory Updated" if updated_memory else None,
        'error': "Failed to update memory" if not updated_memory else None,
        'just_updated': True,
    }
    return render(request, 'memory/memory_main.html', context)


def settings(request):
    """Settings view"""
    from .services import get_available_personalities
    from django.conf import settings as django_settings

    config = load_config()
    if not config:
        return redirect('setup')

    personalities_dir = str(django_settings.PERSONALITIES_DIR)
    available_personalities = get_available_personalities(personalities_dir)
    default_personality = config.get("DEFAULT_PERSONALITY", "assistant")
    model = config.get("MODEL", "")

    # Read first personality file preview
    personality_preview = ""
    selected_personality = default_personality
    if available_personalities:
        selected_personality = request.GET.get('personality', request.GET.get('preview', default_personality))
        personality_path = os.path.join(personalities_dir, selected_personality)
        if os.path.exists(personality_path):
            md_files = [f for f in os.listdir(personality_path) if f.endswith(".md")]
            if md_files:
                with open(os.path.join(personality_path, md_files[0]), 'r') as f:
                    content = f.read()
                    personality_preview = content

    context = {
        'model': model,
        'personalities': available_personalities,
        'default_personality': default_personality,
        'selected_personality': selected_personality,
        'personality_preview': personality_preview,
        'success': request.GET.get('success'),
    }

    # Return partial for HTMX requests, redirect others to chat
    if request.headers.get('HX-Request'):
        return render(request, 'settings/settings_main.html', context)

    return redirect('chat')


def save_settings(request):
    """Save settings (POST)"""
    from .services import get_available_personalities
    from django.conf import settings as django_settings

    if request.method == 'POST':
        selected_personality = request.POST.get('personality')
        config = load_config()
        success_msg = None

        if selected_personality and selected_personality != config.get("DEFAULT_PERSONALITY"):
            config["DEFAULT_PERSONALITY"] = selected_personality
            save_config(config)
            success_msg = "Default personality updated"

        # For HTMX requests, return the partial directly
        if request.headers.get('HX-Request'):
            personalities_dir = str(django_settings.PERSONALITIES_DIR)
            available_personalities = get_available_personalities(personalities_dir)
            default_personality = config.get("DEFAULT_PERSONALITY", "assistant")
            model = config.get("MODEL", "")

            # Read personality preview for the newly set default
            personality_preview = ""
            personality_path = os.path.join(personalities_dir, default_personality)
            if os.path.exists(personality_path):
                md_files = [f for f in os.listdir(personality_path) if f.endswith(".md")]
                if md_files:
                    with open(os.path.join(personality_path, md_files[0]), 'r') as f:
                        content = f.read()
                        personality_preview = content

            context = {
                'model': model,
                'personalities': available_personalities,
                'default_personality': default_personality,
                'selected_personality': default_personality,
                'personality_preview': personality_preview,
                'success': success_msg,
            }
            return render(request, 'settings/settings_main.html', context)

        if success_msg:
            return redirect('settings' + '?success=' + success_msg)

    return redirect('settings')


def save_personality_file(request):
    """Save edited personality file content and optionally rename personality"""
    from django.conf import settings as django_settings
    from .services import get_available_personalities

    if request.method != 'POST':
        return HttpResponse(status=405)

    personality = request.POST.get('personality', '').strip()
    new_name = request.POST.get('new_name', '').strip()
    content = request.POST.get('content', '')

    if not personality:
        return HttpResponse("Personality name required", status=400)

    config = load_config()
    personalities_dir = str(django_settings.PERSONALITIES_DIR)
    old_path = os.path.join(personalities_dir, personality)

    # Determine if we're renaming
    is_rename = new_name and new_name != personality

    if is_rename:
        new_path = os.path.join(personalities_dir, new_name)

        # Validate new name (only alphanumeric and underscores)
        if not all(c.isalnum() or c == '_' for c in new_name):
            return HttpResponse("Invalid personality name. Use only letters, numbers, and underscores.", status=400)

        # Check if new name already exists
        if os.path.exists(new_path):
            return HttpResponse(f"A personality named '{new_name}' already exists.", status=400)

        # Rename the folder
        if os.path.exists(old_path):
            shutil.move(old_path, new_path)

            # Update all session files that reference the old personality
            _update_sessions_personality(personality, new_name)

            # Update config.json if DEFAULT_PERSONALITY matches old name
            if config.get("DEFAULT_PERSONALITY") == personality:
                config["DEFAULT_PERSONALITY"] = new_name
                save_config(config)

            # Use new path for writing content
            personality_path = new_path
            final_personality = new_name
        else:
            return HttpResponse("Original personality not found", status=404)
    else:
        personality_path = old_path
        final_personality = personality

    # Write content to file
    if os.path.exists(personality_path):
        md_files = [f for f in os.listdir(personality_path) if f.endswith(".md")]
        if md_files:
            filepath = os.path.join(personality_path, md_files[0])
            with open(filepath, 'w') as f:
                f.write(content)

    # Reload config in case it was updated
    config = load_config()

    # Return updated settings partial
    available_personalities = get_available_personalities(personalities_dir)
    default_personality = config.get("DEFAULT_PERSONALITY", "assistant")
    model = config.get("MODEL", "")

    context = {
        'model': model,
        'personalities': available_personalities,
        'default_personality': default_personality,
        'selected_personality': final_personality,
        'personality_preview': content,
        'success': "Personality saved" + (" and renamed" if is_rename else ""),
    }
    return render(request, 'settings/settings_main.html', context)


def _update_sessions_personality(old_name, new_name):
    """Update all session files that reference the old personality name"""
    from django.conf import settings as django_settings

    sessions_dir = django_settings.SESSIONS_DIR
    if not os.path.exists(sessions_dir):
        return

    for filename in os.listdir(sessions_dir):
        if filename.endswith('.json'):
            filepath = os.path.join(sessions_dir, filename)
            try:
                with open(filepath, 'r') as f:
                    data = json.load(f)

                if isinstance(data, dict) and data.get('personality') == old_name:
                    data['personality'] = new_name
                    with open(filepath, 'w') as f:
                        json.dump(data, f, indent=4)
            except Exception as e:
                logger.error(f"Error updating session {filename}: {e}")
                continue


def create_personality(request):
    """Create a new personality"""
    from django.conf import settings as django_settings
    from .services import get_available_personalities

    if request.method != 'POST':
        return HttpResponse(status=405)

    name = request.POST.get('name', '').strip()
    content = request.POST.get('content', '')

    if not name:
        return HttpResponse("Personality name required", status=400)

    # Validate name (only alphanumeric and underscores)
    if not all(c.isalnum() or c == '_' for c in name):
        return HttpResponse("Invalid personality name. Use only letters, numbers, and underscores.", status=400)

    config = load_config()
    personalities_dir = str(django_settings.PERSONALITIES_DIR)
    personality_path = os.path.join(personalities_dir, name)

    # Check if already exists
    if os.path.exists(personality_path):
        return HttpResponse(f"A personality named '{name}' already exists.", status=400)

    # Create the folder and identity.md file
    os.makedirs(personality_path)
    filepath = os.path.join(personality_path, 'identity.md')
    with open(filepath, 'w') as f:
        f.write(content)

    # Return updated settings partial with new personality selected
    available_personalities = get_available_personalities(personalities_dir)
    default_personality = config.get("DEFAULT_PERSONALITY", "assistant")
    model = config.get("MODEL", "")

    context = {
        'model': model,
        'personalities': available_personalities,
        'default_personality': default_personality,
        'selected_personality': name,
        'personality_preview': content,
        'success': "Personality created",
    }
    return render(request, 'settings/settings_main.html', context)


def delete_personality(request):
    """Delete a personality"""
    from django.conf import settings as django_settings
    from .services import get_available_personalities

    if request.method != 'POST':
        return HttpResponse(status=405)

    personality = request.POST.get('personality', '').strip()

    if not personality:
        return HttpResponse("Personality name required", status=400)

    config = load_config()
    personalities_dir = str(django_settings.PERSONALITIES_DIR)
    personality_path = os.path.join(personalities_dir, personality)

    # Check if personality exists
    if not os.path.exists(personality_path):
        return HttpResponse("Personality not found", status=404)

    # Get available personalities
    available_personalities = get_available_personalities(personalities_dir)

    # Can't delete if it's the only personality
    if len(available_personalities) <= 1:
        return HttpResponse("Cannot delete the only personality", status=400)

    # Delete the folder
    shutil.rmtree(personality_path)

    # Update config if this was the default personality
    default_personality = config.get("DEFAULT_PERSONALITY", "assistant")
    if default_personality == personality:
        # Set a new default
        available_personalities = get_available_personalities(personalities_dir)
        if available_personalities:
            config["DEFAULT_PERSONALITY"] = available_personalities[0]
            save_config(config)
            default_personality = available_personalities[0]

    # Update sessions that used this personality to use the default
    _update_sessions_personality(personality, default_personality)

    # Reload available personalities after deletion
    available_personalities = get_available_personalities(personalities_dir)
    model = config.get("MODEL", "")

    # Read preview for default personality
    personality_preview = ""
    preview_path = os.path.join(personalities_dir, default_personality)
    if os.path.exists(preview_path):
        md_files = [f for f in os.listdir(preview_path) if f.endswith(".md")]
        if md_files:
            with open(os.path.join(preview_path, md_files[0]), 'r') as f:
                personality_preview = f.read()

    context = {
        'model': model,
        'personalities': available_personalities,
        'default_personality': default_personality,
        'selected_personality': default_personality,
        'personality_preview': personality_preview,
        'success': "Personality deleted",
    }
    return render(request, 'settings/settings_main.html', context)
