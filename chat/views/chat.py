import json
import logging
import os

from django.shortcuts import render, redirect
from django.http import HttpResponse, JsonResponse
from django.conf import settings as django_settings
from django.views.decorators.http import require_POST

from ..services import (
    load_context, get_available_personas, get_persona_model,
    ChatCore, Summarizer,
)
from ..services.memory_worker import (
    start_thread_memory_update, get_thread_update_status,
)
from ..services.session_manager import (
    load_session, create_session, delete_session as delete_session_file,
    toggle_pin, rename_session, save_draft as save_session_draft,
    save_scenario as save_session_scenario,
    clear_draft, remove_last_assistant_message, update_last_user_message,
    get_session_path, generate_session_id, make_user_timestamp,
)
from ..utils import (
    load_config, get_sessions_with_titles,
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


def _build_chat_core(config, session_id, session_persona, session_data=None, user_timezone='UTC'):
    """Build a ChatCore instance for a session. Shared by multiple views."""
    model = get_model_for_persona(config, session_persona, django_settings.PERSONAS_DIR)
    persona_path = os.path.join(str(django_settings.PERSONAS_DIR), session_persona)
    scenario = (session_data or {}).get('scenario', '')
    thread_memory = (session_data or {}).get('thread_memory', '')
    system_prompt = load_context(
        persona_path, persona_name=session_persona,
        scenario=scenario, thread_memory=thread_memory,
    )

    chat_core = ChatCore(
        api_key=config.get("OPENROUTER_API_KEY"),
        model=model,
        site_url=config.get("SITE_URL"),
        site_name=config.get("SITE_NAME"),
        system_prompt=system_prompt,
        context_history_limit=config.get("CONTEXT_HISTORY_LIMIT", 50),
        history_file=str(get_session_path(session_id)),
        persona=session_persona,
        user_timezone=user_timezone
    )
    return chat_core, model


def _build_persona_model_map(available_personas, default_model):
    """Build a {persona: model} mapping for all personas."""
    persona_models = {}
    for p in available_personas:
        pm = get_persona_model(p, str(django_settings.PERSONAS_DIR))
        persona_models[p] = pm or default_model
    return persona_models


def _get_user_timezone(request):
    """Extract and persist user timezone from request."""
    user_timezone = request.POST.get('timezone') or request.session.get('user_timezone', 'UTC')
    if request.method == 'POST' and request.POST.get('timezone'):
        request.session['user_timezone'] = user_timezone
    return user_timezone


def _resolve_session_persona(session_data, config):
    """Get persona from session data, falling back to config default."""
    if session_data:
        persona = session_data.get("persona")
        if persona:
            return persona
    return config.get("DEFAULT_PERSONA", "assistant") or "assistant"


def _handle_title_generation(chat_core, assistant_message, config):
    """
    Handle 3-tier title generation logic. Returns True if title changed.
    """
    if assistant_message.startswith("ERROR:"):
        return False

    first_user_msg = ""
    for msg in chat_core.messages:
        if msg["role"] == "user":
            first_user_msg = msg["content"]
            break

    if not first_user_msg:
        return False

    needs_title = (
        chat_core.title == "New Chat"
        or title_has_artifacts(chat_core.title)
    )

    if needs_title:
        summarizer = Summarizer(
            config.get("OPENROUTER_API_KEY"),
            chat_core.model,
            config.get("SITE_URL"),
            config.get("SITE_NAME"),
        )
        new_title = summarizer.generate_title(first_user_msg, assistant_message)
        chat_core.title = new_title
        chat_core._save_history()
        return True

    return False


def chat(request):
    """Main chat view - session determined from Django session storage"""
    ensure_sessions_dir()
    config = load_config()
    if not config or not config.get("OPENROUTER_API_KEY"):
        return redirect('setup')

    session_id = get_current_session(request)
    is_htmx = request.headers.get('HX-Request') == 'true'

    # For full page loads (refresh), always show home page
    if not is_htmx:
        sessions = get_sessions_with_titles()
        available_personas = get_available_personas(str(django_settings.PERSONAS_DIR))
        default_persona = config.get("DEFAULT_PERSONA", "")
        default_model = config.get("MODEL", "")
        pinned_sessions, grouped_sessions = group_sessions_by_persona(sessions)
        persona_models = _build_persona_model_map(available_personas, default_model)

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
            available_personas = get_available_personas(str(django_settings.PERSONAS_DIR))
            default_persona = config.get("DEFAULT_PERSONA", "")
            default_model = config.get("MODEL", "")
            pinned_sessions, grouped_sessions = group_sessions_by_persona([])
            persona_models = _build_persona_model_map(available_personas, default_model)

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

    # Load session data via SessionManager
    session_data = load_session(session_id)
    session_persona = _resolve_session_persona(session_data, config)
    session_draft = session_data.get("draft", "") if session_data else ""

    user_timezone = _get_user_timezone(request)
    chat_core, model = _build_chat_core(config, session_id, session_persona, session_data, user_timezone)

    # Handle message sending
    if request.method == 'POST' and 'message' in request.POST:
        user_message = request.POST.get('message', '').strip()
        if user_message:
            response = chat_core.send_message(user_message)
            _handle_title_generation(chat_core, response, config)
            return redirect('chat')

    # Prepare sidebar data
    sessions = get_sessions_with_titles()
    pinned_sessions, grouped_sessions = group_sessions_by_persona(sessions)
    collapsed_personas = get_collapsed_personas(request)
    available_personas = get_available_personas(str(django_settings.PERSONAS_DIR))
    default_persona = config.get("DEFAULT_PERSONA", "")

    context = {
        'session_id': session_id,
        'title': chat_core.title,
        'persona': chat_core.persona,
        'model': model,
        'messages': chat_core.messages,
        'draft': session_draft,
        'scenario': session_data.get('scenario', '') if session_data else '',
        'thread_memory': session_data.get('thread_memory', '') if session_data else '',
        'thread_memory_updated_at': session_data.get('thread_memory_updated_at', '') if session_data else '',
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

    if request.headers.get('HX-Request'):
        return render(request, 'chat/chat_main.html', context)

    return render(request, 'chat/chat.html', context)


@require_POST
def switch_session(request):
    """HTMX endpoint to switch current session"""
    session_id = request.POST.get('session_id')
    if session_id:
        set_current_session(request, session_id)

    return chat(request)


def new_chat(request):
    """Show new chat home page (clears current session)"""
    config = load_config()
    if not config:
        return redirect('setup')

    set_current_session(request, None)

    available_personas = get_available_personas(str(django_settings.PERSONAS_DIR))
    default_persona = config.get("DEFAULT_PERSONA", "")
    default_model = config.get("MODEL", "")
    sessions = get_sessions_with_titles()
    pinned_sessions, grouped_sessions = group_sessions_by_persona(sessions)
    persona_models = _build_persona_model_map(available_personas, default_model)

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

    if request.headers.get('HX-Request'):
        return render(request, 'chat/chat_home.html', context)

    return render(request, 'chat/chat.html', {**context, 'show_home': True})


@require_POST
def start_chat(request):
    """Start a new chat - creates session, saves user message, returns chat view with thinking indicator"""

    config = load_config()
    if not config:
        return redirect('setup')

    user_message = request.POST.get('message', '').strip()
    if not user_message:
        return redirect('chat')

    selected_persona = request.POST.get('persona', config.get("DEFAULT_PERSONA", "assistant")) or "assistant"
    session_id = generate_session_id()

    user_timezone = _get_user_timezone(request)
    timestamp = make_user_timestamp(user_timezone)

    initial_messages = [
        {"role": "user", "content": user_message, "timestamp": timestamp}
    ]
    create_session(session_id, selected_persona, messages=initial_messages)

    set_current_session(request, session_id)

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
        'messages': initial_messages,
        'pinned_sessions': pinned_sessions,
        'grouped_sessions': grouped_sessions,
        'current_session': session_id,
        'available_personas': available_personas,
        'default_persona': default_persona,
        'is_htmx': True,
        'pending_message': user_message,
    }

    return render(request, 'chat/chat_main.html', context)


@require_POST
def delete_chat(request):
    """Delete chat session (POST) - supports HTMX for reactive updates"""
    session_id = request.POST.get('session_id')
    if not session_id:
        session_id = get_current_session(request)
    if not session_id:
        return redirect('chat')

    delete_session_file(session_id)

    remaining = [s for s in get_sessions_with_titles() if s["id"] != session_id]

    if request.headers.get('HX-Request'):
        config = load_config()

        if remaining:
            new_session_id = remaining[0]["id"]
            set_current_session(request, new_session_id)

            session_data = load_session(new_session_id)
            session_persona = _resolve_session_persona(session_data, config)

            user_timezone = request.session.get('user_timezone', 'UTC')
            chat_core, model = _build_chat_core(config, new_session_id, session_persona, session_data, user_timezone)

            sessions = get_sessions_with_titles()
            pinned_sessions, grouped_sessions = group_sessions_by_persona(sessions)
            available_personas = get_available_personas(str(django_settings.PERSONAS_DIR))
            default_persona = config.get("DEFAULT_PERSONA", "")

            context = {
                'session_id': new_session_id,
                'title': chat_core.title,
                'persona': chat_core.persona,
                'model': model,
                'messages': chat_core.messages,
                'scenario': session_data.get('scenario', '') if session_data else '',
                'thread_memory': session_data.get('thread_memory', '') if session_data else '',
                'thread_memory_updated_at': session_data.get('thread_memory_updated_at', '') if session_data else '',
                'pinned_sessions': pinned_sessions,
                'grouped_sessions': grouped_sessions,
                'current_session': new_session_id,
                'available_personas': available_personas,
                'default_persona': default_persona,
                'is_htmx': True,
            }

            return render(request, 'chat/chat_main.html', context)
        else:
            set_current_session(request, None)

            sessions = get_sessions_with_titles()
            pinned_sessions, grouped_sessions = group_sessions_by_persona(sessions)
            available_personas = get_available_personas(str(django_settings.PERSONAS_DIR))
            default_persona = config.get("DEFAULT_PERSONA", "") or "assistant"
            default_model = config.get("MODEL", "")
            persona_models = _build_persona_model_map(available_personas, default_model)

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


@require_POST
def toggle_pin_chat(request):
    """Toggle pinned status of a chat session (POST) - returns updated sidebar"""
    session_id = request.POST.get('session_id')
    if not session_id:
        return HttpResponse(status=400)

    result = toggle_pin(session_id)
    if result is None:
        return HttpResponse(status=404)

    sessions = get_sessions_with_titles()
    pinned_sessions, grouped_sessions = group_sessions_by_persona(sessions)
    current_session = get_current_session(request)

    context = {
        'pinned_sessions': pinned_sessions,
        'grouped_sessions': grouped_sessions,
        'current_session': current_session,
    }

    return render(request, 'chat/sidebar_sessions.html', context)


@require_POST
def rename_chat(request):
    """Rename a chat session (POST) - returns updated sidebar"""
    session_id = request.POST.get('session_id')
    new_title = request.POST.get('new_title', '').strip()[:50]

    if not session_id or not new_title:
        return HttpResponse(status=400)

    if not rename_session(session_id, new_title):
        return HttpResponse(status=404)

    sessions = get_sessions_with_titles()
    pinned_sessions, grouped_sessions = group_sessions_by_persona(sessions)
    current_session = get_current_session(request)

    context = {
        'pinned_sessions': pinned_sessions,
        'grouped_sessions': grouped_sessions,
        'current_session': current_session,
    }

    return render(request, 'chat/sidebar_sessions.html', context)


@require_POST
def save_draft(request):
    """Save draft text for a session (POST) - returns minimal response"""
    session_id = request.POST.get('session_id')
    draft = request.POST.get('draft', '')

    if not session_id:
        return HttpResponse(status=400)

    if not save_session_draft(session_id, draft):
        return HttpResponse(status=404)

    return HttpResponse(status=204)


@require_POST
def save_scenario(request):
    """Save scenario text for a session (POST) - returns minimal response"""
    session_id = request.POST.get('session_id')
    scenario = request.POST.get('scenario', '')

    if not session_id:
        return HttpResponse(status=400)

    if not save_session_scenario(session_id, scenario):
        return HttpResponse(status=404)

    return HttpResponse(status=204)


@require_POST
def send_message(request):
    """Send message to chat (HTMX endpoint) - returns HTML fragment"""
    user_message = request.POST.get('message', '').strip()
    if not user_message:
        return HttpResponse(status=400)

    config = load_config()
    if not config or not config.get("OPENROUTER_API_KEY"):
        return HttpResponse('<div class="message error">Configuration error: API key not found</div>')

    is_new_chat = request.POST.get('is_new_chat') == 'true'
    session_id = get_current_session(request)

    if is_new_chat or not session_id:
        selected_persona = request.POST.get('persona', config.get("DEFAULT_PERSONA", "assistant")) or "assistant"
        session_id = generate_session_id()
        create_session(session_id, selected_persona)
        set_current_session(request, session_id)

    # Load session data via SessionManager
    session_data = load_session(session_id)
    session_persona = _resolve_session_persona(session_data, config)

    user_timezone = _get_user_timezone(request)
    chat_core, model = _build_chat_core(config, session_id, session_persona, session_data, user_timezone)

    skip_user_save = request.POST.get('skip_user_save') == 'true'
    assistant_message = chat_core.send_message(user_message, skip_user_save=skip_user_save)

    # Clear draft after successful send
    clear_draft(session_id)

    title_changed = _handle_title_generation(chat_core, assistant_message, config)

    assistant_timestamp = chat_core.messages[-1].get('timestamp', '') if chat_core.messages else ''

    # If this was a new chat, return full chat_main.html
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

    response = render(request, 'chat/assistant_fragment.html', {
        'assistant_message': assistant_message,
        'assistant_timestamp': assistant_timestamp
    })

    if title_changed:
        response['X-Chat-Title'] = chat_core.title
        response['X-Chat-Session-Id'] = session_id

    return response


@require_POST
def retry_message(request):
    """Retry the last assistant message - removes it and resubmits the user message"""
    session_id = get_current_session(request)
    if not session_id:
        return HttpResponse(status=400)

    config = load_config()
    if not config or not config.get("OPENROUTER_API_KEY"):
        return HttpResponse('<div class="message error">Configuration error: API key not found</div>')

    success, user_message, session_data = remove_last_assistant_message(session_id)
    if not success:
        return HttpResponse(status=400)

    session_persona = session_data.get('persona', config.get("DEFAULT_PERSONA", "assistant"))
    user_timezone = request.session.get('user_timezone', 'UTC')
    chat_core, model = _build_chat_core(config, session_id, session_persona, session_data, user_timezone)

    assistant_message = chat_core.send_message(user_message, skip_user_save=True)
    assistant_timestamp = chat_core.messages[-1].get('timestamp', '') if chat_core.messages else ''

    return render(request, 'chat/assistant_fragment.html', {
        'assistant_message': assistant_message,
        'assistant_timestamp': assistant_timestamp
    })


@require_POST
def edit_message(request):
    """Edit the last user message"""
    session_id = get_current_session(request)
    if not session_id:
        return HttpResponse(status=400)

    new_content = request.POST.get('content', '').strip()
    if not new_content:
        return HttpResponse(status=400)

    if not update_last_user_message(session_id, new_content):
        return HttpResponse(status=404)

    return HttpResponse(status=200)


@require_POST
def update_thread_memory(request):
    """Spawn a background thread-memory update for the current session."""
    session_id = request.POST.get('session_id') or get_current_session(request)
    if not session_id:
        return JsonResponse({'error': 'No active session.'}, status=400)

    config = load_config()
    if not config or not config.get('OPENROUTER_API_KEY'):
        return JsonResponse({'error': 'API key not configured.'}, status=400)

    started = start_thread_memory_update(session_id, config)
    if not started:
        return JsonResponse({'state': 'already_running'}, status=409)

    return JsonResponse({'state': 'started'}, status=202)


def thread_memory_status(request):
    """Return the current thread-memory status and content (polling endpoint)."""
    session_id = request.GET.get('session_id') or get_current_session(request)
    if not session_id:
        return JsonResponse({'error': 'No active session.'}, status=400)

    status = get_thread_update_status(session_id)
    session_data = load_session(session_id)
    if session_data:
        status['memory'] = session_data.get('thread_memory', '')
        status['updated_at'] = session_data.get('thread_memory_updated_at', '')
    else:
        status['memory'] = ''
        status['updated_at'] = ''

    return JsonResponse(status)
