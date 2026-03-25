import json
import logging
import os
from datetime import datetime
from zoneinfo import ZoneInfo

from django.shortcuts import render, redirect
from django.http import HttpResponse
from django.conf import settings as django_settings

from ..services import (
    load_context, get_available_personas, get_persona_model,
    ChatCore, Summarizer,
)
from ..utils import (
    load_config, save_config, get_sessions_with_titles,
    group_sessions_by_persona, get_current_session, set_current_session,
    get_collapsed_personas, title_has_artifacts, ensure_sessions_dir,
)
from .core import _get_theme_context

logger = logging.getLogger(__name__)


def get_model_for_persona(config, persona, personas_dir):
    """
    Get the model to use for a persona.
    Returns persona-specific model if set, otherwise the default model.
    """
    default_model = config.get("MODEL", "anthropic/claude-haiku-4.5")
    persona_model = get_persona_model(persona, str(personas_dir))
    return persona_model or default_model


def chat(request):
    """Main chat view - session determined from Django session storage"""
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
        sessions = get_sessions_with_titles()
        available_personas = get_available_personas(str(django_settings.PERSONAS_DIR))
        default_persona = config.get("DEFAULT_PERSONA", "")
        default_model = config.get("MODEL", "")
        pinned_sessions, grouped_sessions = group_sessions_by_persona(sessions)

        # Build persona -> model mapping
        persona_models = {}
        for p in available_personas:
            pm = get_persona_model(p, str(django_settings.PERSONAS_DIR))
            persona_models[p] = pm or default_model

        context = {
            'personas': available_personas,
            'default_persona': default_persona,
            'default_model': default_model,
            'persona_models_json': json.dumps(persona_models),
            'pinned_sessions': pinned_sessions,
            'grouped_sessions': grouped_sessions,
            'current_session': None,
            'is_htmx': False,
            **_get_theme_context(config),
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
            available_personas = get_available_personas(str(django_settings.PERSONAS_DIR))
            default_persona = config.get("DEFAULT_PERSONA", "")
            default_model = config.get("MODEL", "")
            pinned_sessions, grouped_sessions = group_sessions_by_persona([])

            # Build persona -> model mapping
            persona_models = {}
            for p in available_personas:
                pm = get_persona_model(p, str(django_settings.PERSONAS_DIR))
                persona_models[p] = pm or default_model

            context = {
                'personas': available_personas,
                'default_persona': default_persona,
                'default_model': default_model,
                'persona_models_json': json.dumps(persona_models),
                'pinned_sessions': pinned_sessions,
                'grouped_sessions': grouped_sessions,
                'current_session': None,
                'is_htmx': True,
            }
            return render(request, 'chat/chat_home.html', context)

    # Load session data
    session_path = django_settings.SESSIONS_DIR / session_id
    personas_dir = str(django_settings.PERSONAS_DIR)
    api_key = config.get("OPENROUTER_API_KEY")
    context_history_limit = config.get("CONTEXT_HISTORY_LIMIT", 50)
    site_url = config.get("SITE_URL")
    site_name = config.get("SITE_NAME")

    # Try to load persona and draft from session file
    session_persona = None
    session_draft = ''
    if os.path.exists(session_path):
        try:
            with open(session_path, 'r') as f:
                data = json.load(f)
                if isinstance(data, dict):
                    session_persona = data.get("persona")
                    session_draft = data.get("draft", '')
        except:
            pass

    # Fallback to default
    if not session_persona:
        session_persona = config.get("DEFAULT_PERSONA", "assistant") or "assistant"

    # Get model for this persona (may be persona-specific or default)
    model = get_model_for_persona(config, session_persona, django_settings.PERSONAS_DIR)

    # Capture user timezone from POST or session
    user_timezone = request.POST.get('timezone') or request.session.get('user_timezone', 'UTC')
    if request.method == 'POST' and request.POST.get('timezone'):
        request.session['user_timezone'] = user_timezone

    # Load context and create ChatCore
    persona_path = os.path.join(personas_dir, session_persona)
    system_prompt = load_context(persona_path, persona_name=session_persona)

    chat_core = ChatCore(
        api_key=api_key,
        model=model,
        site_url=site_url,
        site_name=site_name,
        system_prompt=system_prompt,
        context_history_limit=context_history_limit,
        history_file=str(session_path),
        persona=session_persona,
        user_timezone=user_timezone
    )

    # Handle message sending
    if request.method == 'POST' and 'message' in request.POST:
        user_message = request.POST.get('message', '').strip()
        if user_message:
            response = chat_core.send_message(user_message)

            # Handle title generation (3-tier logic)
            summarizer = Summarizer(api_key, model, site_url, site_name)

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
    pinned_sessions, grouped_sessions = group_sessions_by_persona(sessions)
    collapsed_personas = get_collapsed_personas(request)

    # Get available personalities for new chat modal
    available_personas = get_available_personas(str(django_settings.PERSONAS_DIR))
    default_persona = config.get("DEFAULT_PERSONA", "")

    context = {
        'session_id': session_id,
        'title': chat_core.title,
        'persona': chat_core.persona,
        'model': model,
        'messages': chat_core.messages,
        'draft': session_draft,
        'sessions': sessions,
        'pinned_sessions': pinned_sessions,
        'grouped_sessions': grouped_sessions,
        'collapsed_personas': collapsed_personas,
        'current_session': session_id,
        'available_personas': available_personas,
        'default_persona': default_persona,
        'is_htmx': request.headers.get('HX-Request') == 'true',
        **_get_theme_context(config),
    }

    # Check if HTMX request - return partial template for sidebar session switching
    if request.headers.get('HX-Request'):
        return render(request, 'chat/chat_main.html', context)

    return render(request, 'chat/chat.html', context)


def switch_session(request):
    """HTMX endpoint to switch current session"""
    if request.method == 'POST':
        session_id = request.POST.get('session_id')
        if session_id:
            set_current_session(request, session_id)

    # Return the chat main partial (reuses chat view logic)
    return chat(request)


def new_chat(request):
    """Show new chat home page (clears current session)"""
    config = load_config()
    if not config:
        return redirect('setup')

    # Clear current session to show home page
    set_current_session(request, None)

    # Get data for home page
    available_personas = get_available_personas(str(django_settings.PERSONAS_DIR))
    default_persona = config.get("DEFAULT_PERSONA", "")
    default_model = config.get("MODEL", "")
    sessions = get_sessions_with_titles()
    pinned_sessions, grouped_sessions = group_sessions_by_persona(sessions)

    # Build persona -> model mapping
    persona_models = {}
    for p in available_personas:
        pm = get_persona_model(p, str(django_settings.PERSONAS_DIR))
        persona_models[p] = pm or default_model

    context = {
        'personas': available_personas,
        'default_persona': default_persona,
        'default_model': default_model,
        'persona_models_json': json.dumps(persona_models),
        'pinned_sessions': pinned_sessions,
        'grouped_sessions': grouped_sessions,
        'current_session': None,
        'is_htmx': request.headers.get('HX-Request') == 'true',
        **_get_theme_context(config),
    }

    # For HTMX requests, return just the home partial
    if request.headers.get('HX-Request'):
        return render(request, 'chat/chat_home.html', context)

    return render(request, 'chat/chat.html', {**context, 'show_home': True})


def start_chat(request):
    """Start a new chat - creates session, saves user message, returns chat view with thinking indicator"""
    if request.method != 'POST':
        return redirect('chat')

    config = load_config()
    if not config:
        return redirect('setup')

    user_message = request.POST.get('message', '').strip()
    if not user_message:
        return redirect('chat')

    # Get persona from form
    selected_persona = request.POST.get('persona', config.get("DEFAULT_PERSONA", "assistant")) or "assistant"

    # Create new session
    session_id = f"session_{datetime.now().strftime('%Y%m%d_%H%M%S')}.json"
    session_path = django_settings.SESSIONS_DIR / session_id

    # Get user timezone
    user_timezone = request.POST.get('timezone') or request.session.get('user_timezone', 'UTC')
    if request.POST.get('timezone'):
        request.session['user_timezone'] = user_timezone

    # Create timestamp for user message
    try:
        tz = ZoneInfo(user_timezone)
    except:
        tz = ZoneInfo('UTC')
    timestamp = datetime.now(tz).isoformat()

    # Create session with user message
    initial_data = {
        "title": "New Chat",
        "persona": selected_persona,
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
    pinned_sessions, grouped_sessions = group_sessions_by_persona(sessions)
    available_personas = get_available_personas(str(django_settings.PERSONAS_DIR))
    default_persona = config.get("DEFAULT_PERSONA", "")
    model = get_model_for_persona(config, selected_persona, django_settings.PERSONAS_DIR)

    context = {
        'session_id': session_id,
        'title': 'New Chat',
        'persona': selected_persona,
        'model': model,
        'messages': initial_data['messages'],
        'pinned_sessions': pinned_sessions,
        'grouped_sessions': grouped_sessions,
        'current_session': session_id,
        'available_personas': available_personas,
        'default_persona': default_persona,
        'is_htmx': True,
        'pending_message': user_message,  # Signal to show thinking indicator and auto-trigger LLM
    }

    return render(request, 'chat/chat_main.html', context)


def delete_chat(request):
    """Delete chat session (POST) - supports HTMX for reactive updates"""
    if request.method == 'POST':
        # Get session_id from POST data or fall back to current session
        session_id = request.POST.get('session_id')
        if not session_id:
            session_id = get_current_session(request)
        if not session_id:
            return redirect('chat')

        # Delete the session file
        session_path = django_settings.SESSIONS_DIR / session_id
        if os.path.exists(session_path):
            os.remove(session_path)

        # Switch to another session or show home page
        remaining = [s for s in get_sessions_with_titles() if s["id"] != session_id]

        # For HTMX requests, return updated main content + sidebar OOB
        if request.headers.get('HX-Request'):
            config = load_config()
            personas_dir = str(django_settings.PERSONAS_DIR)

            if remaining:
                # Switch to another existing session
                new_session_id = remaining[0]["id"]
                set_current_session(request, new_session_id)

                api_key = config.get("OPENROUTER_API_KEY")
                context_history_limit = config.get("CONTEXT_HISTORY_LIMIT", 50)
                site_url = config.get("SITE_URL")
                site_name = config.get("SITE_NAME")

                # Load new session's persona
                new_session_path = django_settings.SESSIONS_DIR / new_session_id
                session_persona = None
                if os.path.exists(new_session_path):
                    try:
                        with open(new_session_path, 'r') as f:
                            data = json.load(f)
                            if isinstance(data, dict):
                                session_persona = data.get("persona")
                    except:
                        pass

                if not session_persona:
                    session_persona = config.get("DEFAULT_PERSONA", "") or "assistant"

                # Get model for this persona (may be persona-specific or default)
                model = get_model_for_persona(config, session_persona, django_settings.PERSONAS_DIR)

                # Load context and create ChatCore for new session
                persona_path = os.path.join(personas_dir, session_persona)
                system_prompt = load_context(persona_path, persona_name=session_persona)
                user_timezone = request.session.get('user_timezone', 'UTC')

                chat_core = ChatCore(
                    api_key=api_key,
                    model=model,
                    site_url=site_url,
                    site_name=site_name,
                    system_prompt=system_prompt,
                    context_history_limit=context_history_limit,
                    history_file=str(new_session_path),
                    persona=session_persona,
                    user_timezone=user_timezone
                )

                # Build context for template
                sessions = get_sessions_with_titles()
                pinned_sessions, grouped_sessions = group_sessions_by_persona(sessions)
                available_personas = get_available_personas(personas_dir)
                default_persona = config.get("DEFAULT_PERSONA", "")

                context = {
                    'session_id': new_session_id,
                    'title': chat_core.title,
                    'persona': chat_core.persona,
                    'model': model,
                    'messages': chat_core.messages,
                    'pinned_sessions': pinned_sessions,
                    'grouped_sessions': grouped_sessions,
                    'current_session': new_session_id,
                    'available_personas': available_personas,
                    'default_persona': default_persona,
                    'is_htmx': True,
                }

                return render(request, 'chat/chat_main.html', context)
            else:
                # No sessions remaining - show home page
                set_current_session(request, None)

                sessions = get_sessions_with_titles()
                pinned_sessions, grouped_sessions = group_sessions_by_persona(sessions)
                available_personas = get_available_personas(personas_dir)
                default_persona = config.get("DEFAULT_PERSONA", "") or "assistant"
                default_model = config.get("MODEL", "")

                # Build persona -> model mapping
                persona_models = {}
                for p in available_personas:
                    pm = get_persona_model(p, personas_dir)
                    persona_models[p] = pm or default_model

                context = {
                    'personas': available_personas,
                    'default_persona': default_persona,
                    'default_model': default_model,
                    'persona_models_json': json.dumps(persona_models),
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
    if request.method != 'POST':
        return HttpResponse(status=405)

    session_id = request.POST.get('session_id')
    if not session_id:
        return HttpResponse(status=400)

    session_path = django_settings.SESSIONS_DIR / session_id
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
    pinned_sessions, grouped_sessions = group_sessions_by_persona(sessions)
    current_session = get_current_session(request)

    context = {
        'pinned_sessions': pinned_sessions,
        'grouped_sessions': grouped_sessions,
        'current_session': current_session,
    }

    return render(request, 'chat/sidebar_sessions.html', context)


def rename_chat(request):
    """Rename a chat session (POST) - returns updated sidebar"""
    if request.method != 'POST':
        return HttpResponse(status=405)

    session_id = request.POST.get('session_id')
    new_title = request.POST.get('new_title', '').strip()[:50]  # 50 char limit

    if not session_id or not new_title:
        return HttpResponse(status=400)

    session_path = django_settings.SESSIONS_DIR / session_id
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
    pinned_sessions, grouped_sessions = group_sessions_by_persona(sessions)
    current_session = get_current_session(request)

    context = {
        'pinned_sessions': pinned_sessions,
        'grouped_sessions': grouped_sessions,
        'current_session': current_session,
    }

    return render(request, 'chat/sidebar_sessions.html', context)


def save_draft(request):
    """Save draft text for a session (POST) - returns minimal response"""
    if request.method != 'POST':
        return HttpResponse(status=405)

    session_id = request.POST.get('session_id')
    draft = request.POST.get('draft', '')

    if not session_id:
        return HttpResponse(status=400)

    session_path = django_settings.SESSIONS_DIR / session_id
    if not os.path.exists(session_path):
        return HttpResponse(status=404)

    try:
        with open(session_path, 'r') as f:
            data = json.load(f)

        data['draft'] = draft

        with open(session_path, 'w') as f:
            json.dump(data, f, indent=2)
    except Exception as e:
        return HttpResponse(f"Error: {e}", status=500)

    return HttpResponse(status=204)  # No content


def send_message(request):
    """Send message to chat (HTMX endpoint) - returns HTML fragment"""
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
        selected_persona = request.POST.get('persona', config.get("DEFAULT_PERSONA", "assistant")) or "assistant"
        session_id = f"session_{datetime.now().strftime('%Y%m%d_%H%M%S')}.json"

        # Create initial session file
        session_path = django_settings.SESSIONS_DIR / session_id
        initial_data = {
            "title": "New Chat",
            "persona": selected_persona,
            "messages": []
        }
        with open(session_path, 'w') as f:
            json.dump(initial_data, f)

        # Set as current session
        set_current_session(request, session_id)

    # Load session data (same as chat view)
    session_path = django_settings.SESSIONS_DIR / session_id
    personas_dir = str(django_settings.PERSONAS_DIR)
    api_key = config.get("OPENROUTER_API_KEY")
    context_history_limit = config.get("CONTEXT_HISTORY_LIMIT", 50)
    site_url = config.get("SITE_URL")
    site_name = config.get("SITE_NAME")

    # Load persona from session file
    session_persona = None
    if os.path.exists(session_path):
        try:
            with open(session_path, 'r') as f:
                data = json.load(f)
                if isinstance(data, dict):
                    session_persona = data.get("persona")
        except:
            pass

    if not session_persona:
        session_persona = config.get("DEFAULT_PERSONA", "assistant") or "assistant"

    # Get model for this persona (may be persona-specific or default)
    model = get_model_for_persona(config, session_persona, django_settings.PERSONAS_DIR)

    # Capture user timezone from POST or session
    user_timezone = request.POST.get('timezone') or request.session.get('user_timezone', 'UTC')
    if request.POST.get('timezone'):
        request.session['user_timezone'] = user_timezone

    # Load context and create ChatCore
    persona_path = os.path.join(personas_dir, session_persona)
    system_prompt = load_context(persona_path, persona_name=session_persona)

    chat_core = ChatCore(
        api_key=api_key,
        model=model,
        site_url=site_url,
        site_name=site_name,
        system_prompt=system_prompt,
        context_history_limit=context_history_limit,
        history_file=str(session_path),
        persona=session_persona,
        user_timezone=user_timezone
    )

    # Check if we should skip saving user message (already saved by start_chat)
    skip_user_save = request.POST.get('skip_user_save') == 'true'

    # Send message and get response
    assistant_message = chat_core.send_message(user_message, skip_user_save=skip_user_save)

    # Clear draft after successful send
    try:
        with open(session_path, 'r') as f:
            data = json.load(f)
        if 'draft' in data:
            data['draft'] = ''
            with open(session_path, 'w') as f:
                json.dump(data, f, indent=2)
    except:
        pass  # Ignore errors clearing draft

    # Handle title generation (same logic as chat view)
    summarizer = Summarizer(api_key, model, site_url, site_name)
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
        sessions = get_sessions_with_titles()
        pinned_sessions, grouped_sessions = group_sessions_by_persona(sessions)
        available_personas = get_available_personas(str(django_settings.PERSONAS_DIR))
        default_persona = config.get("DEFAULT_PERSONA", "")

        context = {
            'session_id': session_id,
            'title': chat_core.title,
            'persona': chat_core.persona,
            'model': model,
            'messages': chat_core.messages,
            'pinned_sessions': pinned_sessions,
            'grouped_sessions': grouped_sessions,
            'current_session': session_id,
            'available_personas': available_personas,
            'default_persona': default_persona,
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


def retry_message(request):
    """Retry the last assistant message - removes it and resubmits the user message"""
    if request.method != 'POST':
        return HttpResponse(status=405)

    session_id = get_current_session(request)
    if not session_id:
        return HttpResponse(status=400)

    config = load_config()
    if not config or not config.get("OPENROUTER_API_KEY"):
        return HttpResponse('<div class="message error">Configuration error: API key not found</div>')

    session_path = django_settings.SESSIONS_DIR / session_id

    # Load session and remove last assistant message
    try:
        with open(session_path, 'r') as f:
            session_data = json.load(f)
    except:
        return HttpResponse(status=404)

    messages = session_data.get('messages', [])
    if len(messages) < 2:
        return HttpResponse(status=400)  # Need at least user + assistant

    # Verify last message is assistant
    if messages[-1].get('role') != 'assistant':
        return HttpResponse(status=400)

    # Remove last assistant message
    messages.pop()

    # Get the last user message (should now be the last message)
    if messages[-1].get('role') != 'user':
        return HttpResponse(status=400)

    user_message = messages[-1].get('content', '')

    # Save session with assistant message removed
    session_data['messages'] = messages
    with open(session_path, 'w') as f:
        json.dump(session_data, f, indent=2)

    # Now resend the user message
    session_persona = session_data.get('persona', config.get("DEFAULT_PERSONA", "assistant"))
    model = get_model_for_persona(config, session_persona, django_settings.PERSONAS_DIR)
    api_key = config.get("OPENROUTER_API_KEY")
    context_history_limit = config.get("CONTEXT_HISTORY_LIMIT", 50)
    site_url = config.get("SITE_URL")
    site_name = config.get("SITE_NAME")

    persona_path = os.path.join(str(django_settings.PERSONAS_DIR), session_persona)
    system_prompt = load_context(persona_path, persona_name=session_persona)

    user_timezone = request.session.get('user_timezone', 'UTC')

    chat_core = ChatCore(
        api_key=api_key,
        model=model,
        site_url=site_url,
        site_name=site_name,
        system_prompt=system_prompt,
        context_history_limit=context_history_limit,
        history_file=str(session_path),
        persona=session_persona,
        user_timezone=user_timezone
    )

    # Send message with skip_user_save since user message is already in history
    assistant_message = chat_core.send_message(user_message, skip_user_save=True)

    # Get assistant timestamp
    assistant_timestamp = chat_core.messages[-1].get('timestamp', '') if chat_core.messages else ''

    return render(request, 'chat/assistant_fragment.html', {
        'assistant_message': assistant_message,
        'assistant_timestamp': assistant_timestamp
    })


def edit_message(request):
    """Edit the last user message"""
    if request.method != 'POST':
        return HttpResponse(status=405)

    session_id = get_current_session(request)
    if not session_id:
        return HttpResponse(status=400)

    new_content = request.POST.get('content', '').strip()
    if not new_content:
        return HttpResponse(status=400)

    session_path = django_settings.SESSIONS_DIR / session_id

    # Load session
    try:
        with open(session_path, 'r') as f:
            session_data = json.load(f)
    except:
        return HttpResponse(status=404)

    messages = session_data.get('messages', [])
    if not messages:
        return HttpResponse(status=400)

    # Find the last user message
    last_user_idx = None
    for i in range(len(messages) - 1, -1, -1):
        if messages[i].get('role') == 'user':
            last_user_idx = i
            break

    if last_user_idx is None:
        return HttpResponse(status=400)

    # Update the message content
    messages[last_user_idx]['content'] = new_content

    # Save session
    session_data['messages'] = messages
    with open(session_path, 'w') as f:
        json.dump(session_data, f, indent=2)

    return HttpResponse(status=200)
