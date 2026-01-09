from django.shortcuts import render, redirect
from django.http import HttpResponse
import sys
import os
import json
import logging

# Add parent directory to path to import our modules
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

logger = logging.getLogger(__name__)

from config_manager import fetch_available_models
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
    from context_manager import load_context, get_available_personalities
    from chat_core import ChatCore
    from summarizer import Summarizer
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
    from context_manager import get_available_personalities
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

    # GET request - show form (will be modal in Phase 4)
    return render(request, 'chat/new_chat.html', {
        'personalities': available_personalities,
        'default_personality': default_personality
    })


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
    from context_manager import load_context
    from chat_core import ChatCore
    from summarizer import Summarizer
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

    return render(request, 'memory/memory.html', context)


def update_memory(request):
    """Update long-term memory (POST)"""
    from django.conf import settings
    from summarizer import Summarizer
    from .utils import aggregate_all_sessions_messages

    if request.method == 'POST':
        config = load_config()
        ltm_file = settings.LTM_FILE
        api_key = config.get("OPENROUTER_API_KEY")
        model = config.get("MODEL")

        try:
            # Aggregate messages from all sessions
            all_messages = aggregate_all_sessions_messages()

            if not all_messages:
                return redirect('memory' + '?error=No messages found in any session')

            # Update memory
            summarizer = Summarizer(api_key, model)
            summarizer.update_long_term_memory(all_messages, ltm_file)

            return redirect('memory' + '?success=Memory updated successfully')

        except Exception as e:
            return redirect('memory' + f'?error=Memory update failed: {str(e)}')

    return redirect('memory')


def wipe_memory(request):
    """Wipe long-term memory (POST)"""
    from django.conf import settings

    if request.method == 'POST':
        ltm_file = settings.LTM_FILE
        if os.path.exists(ltm_file):
            os.remove(ltm_file)
        return redirect('memory' + '?success=Memory wiped successfully')

    return redirect('memory')


def settings(request):
    """Settings view"""
    from context_manager import get_available_personalities
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
    if available_personalities:
        selected_personality = request.GET.get('preview', default_personality)
        personality_path = os.path.join(personalities_dir, selected_personality)
        if os.path.exists(personality_path):
            md_files = [f for f in os.listdir(personality_path) if f.endswith(".md")]
            if md_files:
                with open(os.path.join(personality_path, md_files[0]), 'r') as f:
                    content = f.read()
                    personality_preview = content[:500] + ("..." if len(content) > 500 else "")

    context = {
        'model': model,
        'personalities': available_personalities,
        'default_personality': default_personality,
        'personality_preview': personality_preview,
        'success': request.GET.get('success'),
    }

    return render(request, 'settings/settings.html', context)


def save_settings(request):
    """Save settings (POST)"""
    if request.method == 'POST':
        selected_personality = request.POST.get('personality')
        config = load_config()

        if selected_personality and selected_personality != config.get("DEFAULT_PERSONALITY"):
            config["DEFAULT_PERSONALITY"] = selected_personality
            save_config(config)
            return redirect('settings' + '?success=Default personality updated')

    return redirect('settings')
