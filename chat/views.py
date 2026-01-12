from django.shortcuts import render, redirect
from django.http import HttpResponse
import os
import json
import logging
import shutil

logger = logging.getLogger(__name__)

from .services import fetch_available_models, validate_api_key, get_providers
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

    # Step 1: Provider & API Key
    if step == 1:
        providers = get_providers()

        if request.method == 'POST':
            provider = request.POST.get('provider', 'openrouter')
            api_key = request.POST.get('api_key', '').strip()

            if not api_key:
                return render(request, 'setup/step1.html', {
                    'error': 'Please enter an API key',
                    'providers': providers
                })

            # Validate API key based on provider
            if provider == 'openrouter':
                if not validate_api_key(api_key):
                    logger.error("API key validation failed")
                    return render(request, 'setup/step1.html', {
                        'error': 'Invalid API key. Please check your key and try again.',
                        'api_key': api_key,
                        'providers': providers,
                        'selected_provider': provider
                    })

            logger.info(f"API key validated successfully for provider: {provider}")

            # Write partial config.json with provider and API key
            partial_config = {
                "PROVIDER": provider,
                "OPENROUTER_API_KEY": api_key if provider == 'openrouter' else "",
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
            logger.info(f"Provider ({provider}) and API key saved to config.json")

            # Only store step in session - no API key or models
            request.session['setup_step'] = 2
            request.session.modified = True
            logger.info("Advancing to step 2")
            return redirect('setup')

        return render(request, 'setup/step1.html', {
            'providers': providers
        })

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
    is_htmx = request.headers.get('HX-Request') == 'true'

    # For full page loads (refresh), always show home page
    # For HTMX requests (session switching), load the session
    if not is_htmx:
        # Full page load - show home page
        from .services import get_personality_model
        sessions = get_sessions_with_titles()
        available_personalities = get_available_personalities(str(settings.PERSONALITIES_DIR))
        default_personality = config.get("DEFAULT_PERSONALITY", "")
        default_model = config.get("MODEL", "")
        pinned_sessions, grouped_sessions = group_sessions_by_personality(sessions)

        # Build personality -> model mapping
        personality_models = {}
        for p in available_personalities:
            pm = get_personality_model(p, str(settings.PERSONALITIES_DIR))
            personality_models[p] = pm or default_model

        context = {
            'personalities': available_personalities,
            'default_personality': default_personality,
            'default_model': default_model,
            'personality_models_json': json.dumps(personality_models),
            'pinned_sessions': pinned_sessions,
            'grouped_sessions': grouped_sessions,
            'current_session': None,
            'is_htmx': False,
        }
        return render(request, 'chat/chat.html', {**context, 'show_home': True})

    # HTMX request - load requested session or first available
    if not session_id:
        sessions = get_sessions_with_titles()
        if sessions:
            session_id = sessions[0]["id"]
            set_current_session(request, session_id)
        else:
            # No sessions - show home page partial
            from .services import get_personality_model
            available_personalities = get_available_personalities(str(settings.PERSONALITIES_DIR))
            default_personality = config.get("DEFAULT_PERSONALITY", "")
            default_model = config.get("MODEL", "")
            pinned_sessions, grouped_sessions = group_sessions_by_personality([])

            # Build personality -> model mapping
            personality_models = {}
            for p in available_personalities:
                pm = get_personality_model(p, str(settings.PERSONALITIES_DIR))
                personality_models[p] = pm or default_model

            context = {
                'personalities': available_personalities,
                'default_personality': default_personality,
                'default_model': default_model,
                'personality_models_json': json.dumps(personality_models),
                'pinned_sessions': pinned_sessions,
                'grouped_sessions': grouped_sessions,
                'current_session': None,
                'is_htmx': True,
            }
            return render(request, 'chat/chat_home.html', context)

    # Load session data
    session_path = settings.SESSIONS_DIR / session_id
    personalities_dir = str(settings.PERSONALITIES_DIR)
    ltm_file = str(settings.LTM_FILE)
    api_key = config.get("OPENROUTER_API_KEY")
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
        session_personality = config.get("DEFAULT_PERSONALITY", "assistant") or "assistant"

    # Get model for this personality (may be personality-specific or default)
    model = get_model_for_personality(config, session_personality, settings.PERSONALITIES_DIR)

    # Capture user timezone from POST or session
    user_timezone = request.POST.get('timezone') or request.session.get('user_timezone', 'UTC')
    if request.method == 'POST' and request.POST.get('timezone'):
        request.session['user_timezone'] = user_timezone

    # Load context and create ChatCore
    personality_path = os.path.join(personalities_dir, session_personality)
    system_prompt = load_context(personality_path, ltm_file=ltm_file)

    chat_core = ChatCore(
        api_key=api_key,
        model=model,
        site_url=config.get("SITE_URL"),
        site_name=config.get("SITE_NAME"),
        system_prompt=system_prompt,
        max_history=max_history,
        history_file=str(session_path),
        personality=session_personality,
        user_timezone=user_timezone
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
    pinned_sessions, grouped_sessions = group_sessions_by_personality(sessions)
    collapsed_personalities = get_collapsed_personalities(request)

    # Get available personalities for new chat modal
    available_personalities = get_available_personalities(str(settings.PERSONALITIES_DIR))
    default_personality = config.get("DEFAULT_PERSONALITY", "")

    context = {
        'session_id': session_id,
        'title': chat_core.title,
        'personality': chat_core.personality,
        'model': model,
        'messages': chat_core.messages,
        'sessions': sessions,
        'pinned_sessions': pinned_sessions,
        'grouped_sessions': grouped_sessions,
        'collapsed_personalities': collapsed_personalities,
        'current_session': session_id,
        'available_personalities': available_personalities,
        'default_personality': default_personality,
        'is_htmx': request.headers.get('HX-Request') == 'true',
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
    """Show new chat home page (clears current session)"""
    from .services import get_available_personalities, get_personality_model
    from .utils import set_current_session, get_sessions_with_titles, group_sessions_by_personality
    from django.conf import settings

    config = load_config()
    if not config:
        return redirect('setup')

    # Clear current session to show home page
    set_current_session(request, None)

    # Get data for home page
    available_personalities = get_available_personalities(str(settings.PERSONALITIES_DIR))
    default_personality = config.get("DEFAULT_PERSONALITY", "")
    default_model = config.get("MODEL", "")
    sessions = get_sessions_with_titles()
    pinned_sessions, grouped_sessions = group_sessions_by_personality(sessions)

    # Build personality -> model mapping
    personality_models = {}
    for p in available_personalities:
        pm = get_personality_model(p, str(settings.PERSONALITIES_DIR))
        personality_models[p] = pm or default_model

    context = {
        'personalities': available_personalities,
        'default_personality': default_personality,
        'default_model': default_model,
        'personality_models_json': json.dumps(personality_models),
        'pinned_sessions': pinned_sessions,
        'grouped_sessions': grouped_sessions,
        'current_session': None,
        'is_htmx': request.headers.get('HX-Request') == 'true',
    }

    # For HTMX requests, return just the home partial
    if request.headers.get('HX-Request'):
        return render(request, 'chat/chat_home.html', context)

    return render(request, 'chat/chat.html', {**context, 'show_home': True})


def start_chat(request):
    """Start a new chat - creates session, saves user message, returns chat view with thinking indicator"""
    from datetime import datetime
    from .services import load_context, get_available_personalities
    from .utils import set_current_session, get_sessions_with_titles, group_sessions_by_personality
    from django.conf import settings

    if request.method != 'POST':
        return redirect('chat')

    config = load_config()
    if not config:
        return redirect('setup')

    user_message = request.POST.get('message', '').strip()
    if not user_message:
        return redirect('chat')

    # Get personality from form
    selected_personality = request.POST.get('personality', config.get("DEFAULT_PERSONALITY", "assistant")) or "assistant"

    # Create new session
    session_id = f"session_{datetime.now().strftime('%Y%m%d_%H%M%S')}.json"
    session_path = settings.SESSIONS_DIR / session_id

    # Get user timezone
    user_timezone = request.POST.get('timezone') or request.session.get('user_timezone', 'UTC')
    if request.POST.get('timezone'):
        request.session['user_timezone'] = user_timezone

    # Create timestamp for user message
    from zoneinfo import ZoneInfo
    try:
        tz = ZoneInfo(user_timezone)
    except:
        tz = ZoneInfo('UTC')
    timestamp = datetime.now(tz).isoformat()

    # Create session with user message
    initial_data = {
        "title": "New Chat",
        "personality": selected_personality,
        "messages": [
            {"role": "user", "content": user_message, "timestamp": timestamp}
        ]
    }
    with open(session_path, 'w') as f:
        json.dump(initial_data, f)

    # Set as current session
    set_current_session(request, session_id)

    # Build context for chat_main.html
    sessions = get_sessions_with_titles()
    pinned_sessions, grouped_sessions = group_sessions_by_personality(sessions)
    available_personalities = get_available_personalities(str(settings.PERSONALITIES_DIR))
    default_personality = config.get("DEFAULT_PERSONALITY", "")
    model = get_model_for_personality(config, selected_personality, settings.PERSONALITIES_DIR)

    context = {
        'session_id': session_id,
        'title': 'New Chat',
        'personality': selected_personality,
        'model': model,
        'messages': initial_data['messages'],
        'pinned_sessions': pinned_sessions,
        'grouped_sessions': grouped_sessions,
        'current_session': session_id,
        'available_personalities': available_personalities,
        'default_personality': default_personality,
        'is_htmx': True,
        'pending_message': user_message,  # Signal to show thinking indicator and auto-trigger LLM
    }

    return render(request, 'chat/chat_main.html', context)


def delete_chat(request):
    """Delete chat session (POST) - supports HTMX for reactive updates"""
    from django.conf import settings
    from .services import load_context, get_available_personalities, ChatCore
    from .utils import (
        get_sessions_with_titles, set_current_session, get_current_session,
        group_sessions_by_personality
    )
    from datetime import datetime

    if request.method == 'POST':
        # Get session_id from POST data or fall back to current session
        session_id = request.POST.get('session_id')
        if not session_id:
            session_id = get_current_session(request)
        if not session_id:
            return redirect('chat')

        # Delete the session file
        session_path = settings.SESSIONS_DIR / session_id
        if os.path.exists(session_path):
            os.remove(session_path)

        # Switch to another session or show home page
        remaining = [s for s in get_sessions_with_titles() if s["id"] != session_id]

        # For HTMX requests, return updated main content + sidebar OOB
        if request.headers.get('HX-Request'):
            config = load_config()
            personalities_dir = str(settings.PERSONALITIES_DIR)

            if remaining:
                # Switch to another existing session
                new_session_id = remaining[0]["id"]
                set_current_session(request, new_session_id)

                ltm_file = str(settings.LTM_FILE)
                api_key = config.get("OPENROUTER_API_KEY")
                max_history = config.get("MAX_HISTORY", 50)

                # Load new session's personality
                new_session_path = settings.SESSIONS_DIR / new_session_id
                session_personality = None
                if os.path.exists(new_session_path):
                    try:
                        with open(new_session_path, 'r') as f:
                            data = json.load(f)
                            if isinstance(data, dict):
                                session_personality = data.get("personality")
                    except:
                        pass

                if not session_personality:
                    session_personality = config.get("DEFAULT_PERSONALITY", "") or "assistant"

                # Get model for this personality (may be personality-specific or default)
                model = get_model_for_personality(config, session_personality, settings.PERSONALITIES_DIR)

                # Load context and create ChatCore for new session
                personality_path = os.path.join(personalities_dir, session_personality)
                system_prompt = load_context(personality_path, ltm_file=ltm_file)
                user_timezone = request.session.get('user_timezone', 'UTC')

                chat_core = ChatCore(
                    api_key=api_key,
                    model=model,
                    site_url=config.get("SITE_URL"),
                    site_name=config.get("SITE_NAME"),
                    system_prompt=system_prompt,
                    max_history=max_history,
                    history_file=str(new_session_path),
                    personality=session_personality,
                    user_timezone=user_timezone
                )

                # Build context for template
                sessions = get_sessions_with_titles()
                pinned_sessions, grouped_sessions = group_sessions_by_personality(sessions)
                available_personalities = get_available_personalities(personalities_dir)
                default_personality = config.get("DEFAULT_PERSONALITY", "")

                context = {
                    'session_id': new_session_id,
                    'title': chat_core.title,
                    'personality': chat_core.personality,
                    'model': model,
                    'messages': chat_core.messages,
                    'pinned_sessions': pinned_sessions,
                    'grouped_sessions': grouped_sessions,
                    'current_session': new_session_id,
                    'available_personalities': available_personalities,
                    'default_personality': default_personality,
                    'is_htmx': True,
                }

                return render(request, 'chat/chat_main.html', context)
            else:
                # No sessions remaining - show home page
                from .services import get_personality_model
                set_current_session(request, None)

                sessions = get_sessions_with_titles()
                pinned_sessions, grouped_sessions = group_sessions_by_personality(sessions)
                available_personalities = get_available_personalities(personalities_dir)
                default_personality = config.get("DEFAULT_PERSONALITY", "") or "assistant"
                default_model = config.get("MODEL", "")

                # Build personality -> model mapping
                personality_models = {}
                for p in available_personalities:
                    pm = get_personality_model(p, personalities_dir)
                    personality_models[p] = pm or default_model

                context = {
                    'personalities': available_personalities,
                    'default_personality': default_personality,
                    'default_model': default_model,
                    'personality_models_json': json.dumps(personality_models),
                    'pinned_sessions': pinned_sessions,
                    'grouped_sessions': grouped_sessions,
                    'current_session': None,
                    'is_htmx': True,
                }

                return render(request, 'chat/chat_home.html', context)

        return redirect('chat')

    return redirect('chat')


def toggle_pin_chat(request):
    """Toggle pinned status of a chat session (POST) - returns updated sidebar"""
    from django.conf import settings
    from .utils import get_sessions_with_titles, group_sessions_by_personality, get_current_session

    if request.method != 'POST':
        return HttpResponse(status=405)

    session_id = request.POST.get('session_id')
    if not session_id:
        return HttpResponse(status=400)

    session_path = settings.SESSIONS_DIR / session_id
    if not os.path.exists(session_path):
        return HttpResponse(status=404)

    # Load, toggle pinned, save
    try:
        with open(session_path, 'r') as f:
            data = json.load(f)

        data['pinned'] = not data.get('pinned', False)

        with open(session_path, 'w') as f:
            json.dump(data, f, indent=2)
    except Exception as e:
        return HttpResponse(f"Error: {e}", status=500)

    # Return updated sidebar
    sessions = get_sessions_with_titles()
    pinned_sessions, grouped_sessions = group_sessions_by_personality(sessions)
    current_session = get_current_session(request)

    context = {
        'pinned_sessions': pinned_sessions,
        'grouped_sessions': grouped_sessions,
        'current_session': current_session,
    }

    return render(request, 'chat/sidebar_sessions.html', context)


def rename_chat(request):
    """Rename a chat session (POST) - returns updated sidebar"""
    from django.conf import settings
    from .utils import get_sessions_with_titles, group_sessions_by_personality, get_current_session

    if request.method != 'POST':
        return HttpResponse(status=405)

    session_id = request.POST.get('session_id')
    new_title = request.POST.get('new_title', '').strip()[:50]  # 50 char limit

    if not session_id or not new_title:
        return HttpResponse(status=400)

    session_path = settings.SESSIONS_DIR / session_id
    if not os.path.exists(session_path):
        return HttpResponse(status=404)

    # Load, update title, save
    try:
        with open(session_path, 'r') as f:
            data = json.load(f)

        data['title'] = new_title

        with open(session_path, 'w') as f:
            json.dump(data, f, indent=2)
    except Exception as e:
        return HttpResponse(f"Error: {e}", status=500)

    # Return updated sidebar
    sessions = get_sessions_with_titles()
    pinned_sessions, grouped_sessions = group_sessions_by_personality(sessions)
    current_session = get_current_session(request)

    context = {
        'pinned_sessions': pinned_sessions,
        'grouped_sessions': grouped_sessions,
        'current_session': current_session,
    }

    return render(request, 'chat/sidebar_sessions.html', context)


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

    # Load config first (needed for new chat creation)
    config = load_config()
    if not config or not config.get("OPENROUTER_API_KEY"):
        return HttpResponse('<div class="message error">Configuration error: API key not found</div>')

    # Check if this is a new chat from home page
    is_new_chat = request.POST.get('is_new_chat') == 'true'
    session_id = get_current_session(request)

    if is_new_chat or not session_id:
        # Create new session
        from datetime import datetime
        from .utils import set_current_session

        selected_personality = request.POST.get('personality', config.get("DEFAULT_PERSONALITY", "assistant")) or "assistant"
        session_id = f"session_{datetime.now().strftime('%Y%m%d_%H%M%S')}.json"

        # Create initial session file
        session_path = settings.SESSIONS_DIR / session_id
        initial_data = {
            "title": "New Chat",
            "personality": selected_personality,
            "messages": []
        }
        with open(session_path, 'w') as f:
            json.dump(initial_data, f)

        # Set as current session
        set_current_session(request, session_id)

    # Load session data (same as chat view)
    session_path = settings.SESSIONS_DIR / session_id
    personalities_dir = str(settings.PERSONALITIES_DIR)
    ltm_file = str(settings.LTM_FILE)
    api_key = config.get("OPENROUTER_API_KEY")
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
        session_personality = config.get("DEFAULT_PERSONALITY", "assistant") or "assistant"

    # Get model for this personality (may be personality-specific or default)
    model = get_model_for_personality(config, session_personality, settings.PERSONALITIES_DIR)

    # Capture user timezone from POST or session
    user_timezone = request.POST.get('timezone') or request.session.get('user_timezone', 'UTC')
    if request.POST.get('timezone'):
        request.session['user_timezone'] = user_timezone

    # Load context and create ChatCore
    personality_path = os.path.join(personalities_dir, session_personality)
    system_prompt = load_context(personality_path, ltm_file=ltm_file)

    chat_core = ChatCore(
        api_key=api_key,
        model=model,
        site_url=config.get("SITE_URL"),
        site_name=config.get("SITE_NAME"),
        system_prompt=system_prompt,
        max_history=max_history,
        history_file=str(session_path),
        personality=session_personality,
        user_timezone=user_timezone
    )

    # Check if we should skip saving user message (already saved by start_chat)
    skip_user_save = request.POST.get('skip_user_save') == 'true'

    # Send message and get response
    assistant_message = chat_core.send_message(user_message, skip_user_save=skip_user_save)

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

    # Get assistant timestamp from the last message
    assistant_timestamp = chat_core.messages[-1].get('timestamp', '') if chat_core.messages else ''

    # If this was a new chat, return full chat_main.html (targets #main-content)
    if is_new_chat:
        from .utils import get_sessions_with_titles, group_sessions_by_personality
        from .services import get_available_personalities

        sessions = get_sessions_with_titles()
        pinned_sessions, grouped_sessions = group_sessions_by_personality(sessions)
        available_personalities = get_available_personalities(str(settings.PERSONALITIES_DIR))
        default_personality = config.get("DEFAULT_PERSONALITY", "")

        context = {
            'session_id': session_id,
            'title': chat_core.title,
            'personality': chat_core.personality,
            'model': model,
            'messages': chat_core.messages,
            'pinned_sessions': pinned_sessions,
            'grouped_sessions': grouped_sessions,
            'current_session': session_id,
            'available_personalities': available_personalities,
            'default_personality': default_personality,
            'is_htmx': True,
        }
        return render(request, 'chat/chat_main.html', context)

    # Return HTML fragment for HTMX (only assistant message, user already shown)
    response = render(request, 'chat/assistant_fragment.html', {
        'assistant_message': assistant_message,
        'assistant_timestamp': assistant_timestamp
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
    from .services import list_context_files

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

    # Get user context files
    context_files = list_context_files()

    context = {
        'model': model,
        'memory_content': memory_content,
        'last_update': last_update,
        'success': request.GET.get('success'),
        'error': request.GET.get('error'),
        'context_files': context_files,
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
    from .services import Summarizer, list_context_files
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
                'context_files': list_context_files(),
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
    from .services import list_context_files

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
                'context_files': list_context_files(),
            }
            return render(request, 'memory/memory_main.html', context)

        return redirect(f"{reverse('memory')}?success=Memory wiped successfully")

    return redirect('memory')


def modify_memory(request):
    """Modify memory based on user command (HTMX endpoint)"""
    from django.conf import settings
    from datetime import datetime
    from .services import Summarizer, list_context_files

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
        'context_files': list_context_files(),
    }
    return render(request, 'memory/memory_main.html', context)


def upload_context_file(request):
    """Upload a user context file (HTMX/AJAX endpoint)"""
    from datetime import datetime
    from django.conf import settings as django_settings
    from django.http import JsonResponse
    from .services import upload_context_file as do_upload, list_context_files

    if request.method != 'POST':
        return HttpResponse(status=405)

    uploaded_file = request.FILES.get('file')
    if not uploaded_file:
        return HttpResponse("No file provided", status=400)

    # Upload the file
    filename = do_upload(uploaded_file)

    # For AJAX requests (from modal), return JSON
    if request.headers.get('X-Requested-With') == 'XMLHttpRequest':
        return JsonResponse({
            'success': bool(filename),
            'filename': filename,
            'files': list_context_files()
        })

    # For HTMX requests, return HTML partial
    config = load_config()
    ltm_file = django_settings.LTM_FILE
    model = config.get("MODEL", "") if config else ""

    last_update = None
    if os.path.exists(ltm_file):
        last_update = datetime.fromtimestamp(os.path.getmtime(ltm_file))

    memory_content = ""
    if os.path.exists(ltm_file):
        with open(ltm_file, 'r') as f:
            memory_content = f.read()

    context = {
        'model': model,
        'memory_content': memory_content,
        'last_update': last_update,
        'context_files': list_context_files(),
        'success': f"Uploaded {filename}" if filename else None,
        'error': "Invalid file type. Only .md and .txt files allowed." if not filename else None,
    }
    return render(request, 'memory/memory_main.html', context)


def delete_context_file(request):
    """Delete a user context file (HTMX/AJAX endpoint)"""
    from datetime import datetime
    from django.conf import settings as django_settings
    from django.http import JsonResponse
    from .services import delete_context_file as do_delete, list_context_files

    if request.method != 'POST':
        return HttpResponse(status=405)

    filename = request.POST.get('filename', '')
    if not filename:
        return HttpResponse("No filename provided", status=400)

    # Delete the file
    deleted = do_delete(filename)

    # For AJAX requests (from modal), return JSON
    if request.headers.get('X-Requested-With') == 'XMLHttpRequest':
        return JsonResponse({
            'success': deleted,
            'filename': filename,
            'files': list_context_files()
        })

    # For HTMX requests, return HTML partial
    config = load_config()
    ltm_file = django_settings.LTM_FILE
    model = config.get("MODEL", "") if config else ""

    last_update = None
    if os.path.exists(ltm_file):
        last_update = datetime.fromtimestamp(os.path.getmtime(ltm_file))

    memory_content = ""
    if os.path.exists(ltm_file):
        with open(ltm_file, 'r') as f:
            memory_content = f.read()

    context = {
        'model': model,
        'memory_content': memory_content,
        'last_update': last_update,
        'context_files': list_context_files(),
        'success': f"Deleted {filename}" if deleted else None,
        'error': f"File not found: {filename}" if not deleted else None,
    }
    return render(request, 'memory/memory_main.html', context)


def toggle_context_file(request):
    """Toggle enabled status of a user context file (HTMX/AJAX endpoint)"""
    from datetime import datetime
    from django.conf import settings as django_settings
    from django.http import JsonResponse
    from .services import toggle_context_file as do_toggle, list_context_files

    if request.method != 'POST':
        return HttpResponse(status=405)

    filename = request.POST.get('filename', '')
    if not filename:
        return HttpResponse("No filename provided", status=400)

    # Toggle the file
    new_status = do_toggle(filename)

    # For AJAX requests (from modal), return JSON
    if request.headers.get('X-Requested-With') == 'XMLHttpRequest':
        return JsonResponse({
            'success': True,
            'filename': filename,
            'enabled': new_status,
            'files': list_context_files()
        })

    # For HTMX requests, return HTML partial
    config = load_config()
    ltm_file = django_settings.LTM_FILE
    model = config.get("MODEL", "") if config else ""

    last_update = None
    if os.path.exists(ltm_file):
        last_update = datetime.fromtimestamp(os.path.getmtime(ltm_file))

    memory_content = ""
    if os.path.exists(ltm_file):
        with open(ltm_file, 'r') as f:
            memory_content = f.read()

    context = {
        'model': model,
        'memory_content': memory_content,
        'last_update': last_update,
        'context_files': list_context_files(),
    }
    return render(request, 'memory/memory_main.html', context)


def get_context_file_content(request):
    """GET endpoint to retrieve context file content for editing"""
    from django.http import JsonResponse
    from .services import get_user_context_dir

    filename = request.GET.get('filename')
    if not filename:
        return JsonResponse({'error': 'No filename provided'}, status=400)

    file_path = get_user_context_dir() / filename
    if not file_path.exists():
        return JsonResponse({'error': 'File not found'}, status=404)

    content = file_path.read_text()
    return JsonResponse({'filename': filename, 'content': content})


def save_context_file_content(request):
    """POST endpoint to save edited context file content"""
    from django.http import JsonResponse
    from .services import get_user_context_dir

    if request.method != 'POST':
        return JsonResponse({'error': 'POST required'}, status=405)

    filename = request.POST.get('filename')
    content = request.POST.get('content', '')

    if not filename:
        return JsonResponse({'error': 'No filename provided'}, status=400)

    file_path = get_user_context_dir() / filename
    if not file_path.exists():
        return JsonResponse({'error': 'File not found'}, status=404)

    file_path.write_text(content)
    return JsonResponse({'success': True, 'filename': filename})


def settings(request):
    """Settings view"""
    from .services import get_available_personalities, get_personality_model
    from django.conf import settings as django_settings

    config = load_config()
    if not config:
        return redirect('setup')

    personalities_dir = str(django_settings.PERSONALITIES_DIR)
    available_personalities = get_available_personalities(personalities_dir)
    default_personality = config.get("DEFAULT_PERSONALITY", "")
    model = config.get("MODEL", "")
    provider = config.get("PROVIDER", "openrouter")
    providers = get_providers()

    # Check if API key exists for current provider
    has_api_key = False
    api_key = None
    if provider == 'openrouter':
        api_key = config.get("OPENROUTER_API_KEY")
        has_api_key = bool(api_key)

    # Fetch available models for Edit Model modal
    available_models = []
    if has_api_key and api_key:
        models = fetch_available_models(api_key)
        if models:
            grouped = group_models_by_provider(models)
            model_options = flatten_models_with_provider_prefix(grouped)
            available_models = [{'id': m[0], 'display': m[1]} for m in model_options]

    # Read first personality file preview
    personality_preview = ""
    selected_personality = default_personality
    personality_model = None
    if available_personalities:
        selected_personality = request.GET.get('personality', request.GET.get('preview', default_personality))
        # Only load preview if a personality is actually selected
        if selected_personality:
            personality_path = os.path.join(personalities_dir, selected_personality)
            if os.path.exists(personality_path):
                md_files = [f for f in os.listdir(personality_path) if f.endswith(".md")]
                if md_files:
                    with open(os.path.join(personality_path, md_files[0]), 'r') as f:
                        content = f.read()
                        personality_preview = content
            # Get personality-specific model if set
            personality_model = get_personality_model(selected_personality, personalities_dir)

    context = {
        'model': model,
        'provider': provider,
        'providers': providers,
        'providers_json': json.dumps(providers),
        'has_api_key': has_api_key,
        'personalities': available_personalities,
        'default_personality': default_personality,
        'selected_personality': selected_personality,
        'personality_preview': personality_preview,
        'personality_model': personality_model or '',
        'available_models': available_models,
        'available_models_json': json.dumps(available_models),
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
        selected_personality = request.POST.get('personality', '').strip()
        config = load_config()
        success_msg = None

        # Personality is required - fall back to "assistant" if empty
        if not selected_personality:
            selected_personality = "assistant"

        # Update if different from current
        if selected_personality != config.get("DEFAULT_PERSONALITY", ""):
            config["DEFAULT_PERSONALITY"] = selected_personality
            save_config(config)
            success_msg = "Default personality updated"

        # For HTMX requests, return the partial directly
        if request.headers.get('HX-Request'):
            from .services import get_personality_model
            personalities_dir = str(django_settings.PERSONALITIES_DIR)
            available_personalities = get_available_personalities(personalities_dir)
            default_personality = config.get("DEFAULT_PERSONALITY", "")
            model = config.get("MODEL", "")
            provider = config.get("PROVIDER", "openrouter")
            providers = get_providers()

            # Check if API key exists and fetch models
            has_api_key = False
            api_key = None
            available_models = []
            if provider == 'openrouter':
                api_key = config.get("OPENROUTER_API_KEY")
                has_api_key = bool(api_key)
            if has_api_key and api_key:
                models_list = fetch_available_models(api_key)
                if models_list:
                    grouped = group_models_by_provider(models_list)
                    model_options = flatten_models_with_provider_prefix(grouped)
                    available_models = [{'id': m[0], 'display': m[1]} for m in model_options]

            # Read personality preview for the newly set default (if set)
            personality_preview = ""
            personality_model = None
            if default_personality:
                personality_path = os.path.join(personalities_dir, default_personality)
                if os.path.exists(personality_path):
                    md_files = [f for f in os.listdir(personality_path) if f.endswith(".md")]
                    if md_files:
                        with open(os.path.join(personality_path, md_files[0]), 'r') as f:
                            content = f.read()
                            personality_preview = content
                personality_model = get_personality_model(default_personality, personalities_dir)

            context = {
                'model': model,
                'provider': provider,
                'providers': providers,
                'providers_json': json.dumps(providers),
                'has_api_key': has_api_key,
                'personalities': available_personalities,
                'default_personality': default_personality,
                'selected_personality': default_personality,
                'personality_preview': personality_preview,
                'personality_model': personality_model or '',
                'available_models': available_models,
                'available_models_json': json.dumps(available_models),
                'success': success_msg,
            }
            return render(request, 'settings/settings_main.html', context)

        if success_msg:
            return redirect('settings' + '?success=' + success_msg)

    return redirect('settings')


def validate_provider_api_key(request):
    """Validate API key and return models list (JSON endpoint for Settings page)"""
    from django.http import JsonResponse

    if request.method != 'POST':
        return JsonResponse({'error': 'POST required'}, status=405)

    provider = request.POST.get('provider', 'openrouter')
    api_key = request.POST.get('api_key', '').strip()
    use_existing = request.POST.get('use_existing', 'false') == 'true'

    # If using existing key, get it from config
    if use_existing:
        config = load_config()
        if provider == 'openrouter':
            api_key = config.get('OPENROUTER_API_KEY', '')

    if not api_key:
        return JsonResponse({'valid': False, 'error': 'API key required'})

    # Validate based on provider
    if provider == 'openrouter':
        # Skip validation if using existing (already validated)
        if not use_existing and not validate_api_key(api_key):
            return JsonResponse({'valid': False, 'error': 'Invalid API key'})

        # Fetch models for this key
        models = fetch_available_models(api_key)
        if not models:
            return JsonResponse({'valid': False, 'error': 'Could not fetch models'})

        # Format models for frontend
        grouped = group_models_by_provider(models)
        model_options = flatten_models_with_provider_prefix(grouped)

        return JsonResponse({
            'valid': True,
            'models': [{'id': m[0], 'display': m[1]} for m in model_options]
        })

    return JsonResponse({'valid': False, 'error': 'Unknown provider'})


def save_provider_model(request):
    """Save provider and model settings (JSON endpoint for Settings page)"""
    from django.http import JsonResponse

    if request.method != 'POST':
        return JsonResponse({'error': 'POST required'}, status=405)

    provider = request.POST.get('provider', '').strip()
    api_key = request.POST.get('api_key', '').strip()
    model = request.POST.get('model', '').strip()
    keep_existing_key = request.POST.get('keep_existing_key', 'false') == 'true'

    if not provider or not model:
        return JsonResponse({'success': False, 'error': 'Provider and model required'})

    config = load_config()

    # Safety check: if config is empty but we're keeping existing key, file may be corrupted
    from django.conf import settings as django_settings
    if keep_existing_key and not config.get('OPENROUTER_API_KEY'):
        if os.path.exists(django_settings.CONFIG_FILE):
            logger.error("Config appears corrupted - load returned empty but file exists")
            return JsonResponse({'success': False, 'error': 'Configuration file may be corrupted. Please check config.json'})

    # Update provider
    config['PROVIDER'] = provider

    # Update API key (only if new one provided)
    if api_key and not keep_existing_key:
        if provider == 'openrouter':
            config['OPENROUTER_API_KEY'] = api_key

    # Update model
    config['MODEL'] = model

    save_config(config)

    return JsonResponse({
        'success': True,
        'provider': provider,
        'model': model
    })


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
    from .services import get_personality_model
    available_personalities = get_available_personalities(personalities_dir)
    default_personality = config.get("DEFAULT_PERSONALITY", "")
    model = config.get("MODEL", "")
    provider = config.get("PROVIDER", "openrouter")
    providers = get_providers()

    # Check if API key exists and fetch models
    has_api_key = False
    api_key = None
    available_models = []
    if provider == 'openrouter':
        api_key = config.get("OPENROUTER_API_KEY")
        has_api_key = bool(api_key)
    if has_api_key and api_key:
        models_list = fetch_available_models(api_key)
        if models_list:
            grouped = group_models_by_provider(models_list)
            model_options = flatten_models_with_provider_prefix(grouped)
            available_models = [{'id': m[0], 'display': m[1]} for m in model_options]

    # Get personality model
    personality_model = get_personality_model(final_personality, personalities_dir)

    context = {
        'model': model,
        'provider': provider,
        'providers': providers,
        'providers_json': json.dumps(providers),
        'has_api_key': has_api_key,
        'personalities': available_personalities,
        'default_personality': default_personality,
        'selected_personality': final_personality,
        'personality_preview': content,
        'personality_model': personality_model or '',
        'available_models': available_models,
        'available_models_json': json.dumps(available_models),
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
    default_personality = config.get("DEFAULT_PERSONALITY", "")
    model = config.get("MODEL", "")
    provider = config.get("PROVIDER", "openrouter")
    providers = get_providers()

    # Check if API key exists and fetch models
    has_api_key = False
    api_key = None
    available_models = []
    if provider == 'openrouter':
        api_key = config.get("OPENROUTER_API_KEY")
        has_api_key = bool(api_key)
    if has_api_key and api_key:
        models_list = fetch_available_models(api_key)
        if models_list:
            grouped = group_models_by_provider(models_list)
            model_options = flatten_models_with_provider_prefix(grouped)
            available_models = [{'id': m[0], 'display': m[1]} for m in model_options]

    context = {
        'model': model,
        'provider': provider,
        'providers': providers,
        'providers_json': json.dumps(providers),
        'has_api_key': has_api_key,
        'personalities': available_personalities,
        'default_personality': default_personality,
        'selected_personality': name,
        'personality_preview': content,
        'personality_model': '',  # New personality has no model override
        'available_models': available_models,
        'available_models_json': json.dumps(available_models),
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
    default_personality = config.get("DEFAULT_PERSONALITY", "")
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
    from .services import get_personality_model
    available_personalities = get_available_personalities(personalities_dir)
    model = config.get("MODEL", "")
    provider = config.get("PROVIDER", "openrouter")
    providers = get_providers()

    # Check if API key exists and fetch models
    has_api_key = False
    api_key = None
    available_models = []
    if provider == 'openrouter':
        api_key = config.get("OPENROUTER_API_KEY")
        has_api_key = bool(api_key)
    if has_api_key and api_key:
        models_list = fetch_available_models(api_key)
        if models_list:
            grouped = group_models_by_provider(models_list)
            model_options = flatten_models_with_provider_prefix(grouped)
            available_models = [{'id': m[0], 'display': m[1]} for m in model_options]

    # Read preview for default personality
    personality_preview = ""
    preview_path = os.path.join(personalities_dir, default_personality)
    if os.path.exists(preview_path):
        md_files = [f for f in os.listdir(preview_path) if f.endswith(".md")]
        if md_files:
            with open(os.path.join(preview_path, md_files[0]), 'r') as f:
                personality_preview = f.read()

    # Get personality model for the new default
    personality_model = get_personality_model(default_personality, personalities_dir)

    context = {
        'model': model,
        'provider': provider,
        'providers': providers,
        'providers_json': json.dumps(providers),
        'has_api_key': has_api_key,
        'personalities': available_personalities,
        'default_personality': default_personality,
        'selected_personality': default_personality,
        'personality_preview': personality_preview,
        'personality_model': personality_model or '',
        'available_models': available_models,
        'available_models_json': json.dumps(available_models),
        'success': "Personality deleted",
    }
    return render(request, 'settings/settings_main.html', context)


def get_model_for_personality(config, personality, personalities_dir):
    """
    Get the model to use for a personality.
    Returns personality-specific model if set, otherwise the default model.
    """
    from .services import get_personality_model

    default_model = config.get("MODEL", "anthropic/claude-haiku-4.5")
    personality_model = get_personality_model(personality, str(personalities_dir))
    return personality_model or default_model


def save_personality_model(request):
    """Save model override for a personality (POST)"""
    from django.http import JsonResponse
    from django.conf import settings as django_settings
    from .services import get_personality_config

    if request.method != 'POST':
        return JsonResponse({'error': 'Method not allowed'}, status=405)

    personality = request.POST.get('personality', '').strip()
    model = request.POST.get('model', '').strip()

    if not personality:
        return JsonResponse({'error': 'Personality is required'}, status=400)

    # Validate personality exists
    personality_path = django_settings.PERSONALITIES_DIR / personality
    if not personality_path.exists():
        return JsonResponse({'error': 'Personality not found'}, status=404)

    # Load existing config or create new
    config_path = personality_path / "config.json"
    config = {}
    if config_path.exists():
        with open(config_path, 'r') as f:
            config = json.load(f)

    # Update or remove model
    if model:
        config["model"] = model
    elif "model" in config:
        del config["model"]

    # Save config
    with open(config_path, 'w') as f:
        json.dump(config, f, indent=2)

    return JsonResponse({'success': True, 'model': model or None})
