import json
import logging
import os
from datetime import datetime

from django.shortcuts import redirect, render
from django.http import HttpResponse, JsonResponse
from django.conf import settings as django_settings
from django.urls import reverse

from ..services import (
    MemoryManager,
    get_memory_file, get_memory_content, delete_memory, get_memory_model,
    get_available_personas, get_persona_identity,
    list_context_files,
    upload_context_file as do_upload_context,
    delete_context_file as do_delete_context,
    toggle_context_file as do_toggle_context,
    get_user_context_dir,
    list_persona_context_files,
    upload_persona_context_file as do_upload_persona_context,
    delete_persona_context_file as do_delete_persona_context,
    toggle_persona_context_file as do_toggle_persona_context,
    get_persona_context_file_content as do_get_persona_content,
    save_persona_context_file_content as do_save_persona_content,
    add_context_local_directory, remove_context_local_directory,
    list_context_local_directories, toggle_context_local_file,
    get_context_local_file_content, refresh_context_local_directory,
    add_persona_context_local_directory, remove_persona_context_local_directory,
    list_persona_context_local_directories, toggle_persona_context_local_file,
    get_persona_context_local_file_content, refresh_persona_context_local_directory,
    browse_directory,
)
from ..utils import load_config, save_config, aggregate_all_sessions_messages

logger = logging.getLogger(__name__)


def _context_badge_count():
    """Count enabled uploaded files + enabled local directory files."""
    files = list_context_files()
    enabled_uploaded = sum(1 for f in files if f.get('enabled'))
    local_dirs = list_context_local_directories()
    enabled_local = sum(
        1 for d in local_dirs for f in d.get('files', []) if f.get('enabled')
    )
    return enabled_uploaded + enabled_local


def _memory_context(context_files):
    """Add local directory data to a memory template context dict."""
    local_dirs = list_context_local_directories()
    return {
        'context_local_dirs_json': json.dumps(local_dirs),
        'context_badge_count': _context_badge_count(),
    }


def _build_memory_view_context(config, selected_persona, success=None, error=None, just_updated=False):
    """Build the full context dict for memory_main.html."""
    memory_content = get_memory_content(selected_persona)
    memory_file = get_memory_file(selected_persona)

    last_update = None
    if memory_file.exists():
        last_update = datetime.fromtimestamp(os.path.getmtime(memory_file))

    ctx_files = list_context_files()
    return {
        'model': config.get("MODEL", ""),
        'selected_persona': selected_persona,
        'available_personas': get_available_personas(str(django_settings.PERSONAS_DIR)),
        'memory_content': memory_content,
        'last_update': last_update,
        'success': success,
        'error': error,
        'just_updated': just_updated,
        'context_files': ctx_files,
        'memory_size_limit': config.get('MEMORY_SIZE_LIMIT', 8000),
        'user_history_max_threads': config.get('USER_HISTORY_MAX_THREADS', 10),
        'user_history_messages_per_thread': config.get('USER_HISTORY_MESSAGES_PER_THREAD', 100),
        **_memory_context(ctx_files),
    }


def memory(request):
    """User memory view"""
    config = load_config()
    if not config:
        return redirect('setup')

    selected_persona = request.GET.get('persona', config.get("DEFAULT_PERSONA", "assistant"))

    context = _build_memory_view_context(
        config, selected_persona,
        success=request.GET.get('success'),
        error=request.GET.get('error'),
    )

    if request.headers.get('HX-Request'):
        return render(request, 'memory/memory_main.html', context)

    return redirect('chat')


def update_memory(request):
    """Update per-persona memory (POST)"""
    if request.method == 'POST':
        config = load_config()
        selected_persona = request.POST.get('persona', config.get("DEFAULT_PERSONA", "assistant"))
        api_key = config.get("OPENROUTER_API_KEY")
        site_url = config.get("SITE_URL")
        site_name = config.get("SITE_NAME")

        success_msg = None
        error_msg = None

        try:
            user_history_max_threads = config.get('USER_HISTORY_MAX_THREADS', 10)
            user_history_messages_per_thread = config.get('USER_HISTORY_MESSAGES_PER_THREAD', 100)

            threads = aggregate_all_sessions_messages(
                user_history_max_threads=user_history_max_threads if user_history_max_threads > 0 else None,
                user_history_messages_per_thread=user_history_messages_per_thread if user_history_messages_per_thread > 0 else None,
                persona_filter=selected_persona,
            )

            if not threads:
                persona_display = selected_persona.replace('_', ' ').title()
                error_msg = f"No conversations found for {persona_display}. Chat with this persona first to build memory."
            else:
                persona_dir = str(django_settings.PERSONAS_DIR / selected_persona)
                persona_identity = get_persona_identity(persona_dir)
                memory_model = get_memory_model(config, selected_persona, str(django_settings.PERSONAS_DIR))
                size_limit = config.get('MEMORY_SIZE_LIMIT', 8000)

                manager = MemoryManager(api_key, memory_model, site_url, site_name)
                if manager.update_persona_memory(selected_persona, persona_identity, threads, size_limit):
                    success_msg = "Memory Updated"
                else:
                    error_msg = "Memory update failed — the model returned an unusable response."

        except Exception as e:
            error_msg = f"Memory update failed: {str(e)}"

        if request.headers.get('HX-Request'):
            context = _build_memory_view_context(
                config, selected_persona,
                success=success_msg, error=error_msg,
                just_updated=bool(success_msg),
            )
            return render(request, 'memory/memory_main.html', context)

        if error_msg:
            return redirect(f"{reverse('memory')}?error={error_msg}")
        return redirect(f"{reverse('memory')}?success={success_msg}")

    return redirect('memory')


def save_memory_settings(request):
    """Save memory generation settings (AJAX endpoint)"""
    if request.method != 'POST':
        return HttpResponse(status=405)

    config = load_config()

    user_history_max_threads = request.POST.get('user_history_max_threads', 0)
    user_history_messages_per_thread = request.POST.get('user_history_messages_per_thread', 0)
    memory_size_limit = request.POST.get('memory_size_limit', 8000)

    try:
        user_history_max_threads = int(user_history_max_threads)
        user_history_messages_per_thread = int(user_history_messages_per_thread)
        memory_size_limit = int(memory_size_limit)
        # Clamp to reasonable values (0 = unlimited)
        user_history_max_threads = max(0, min(100, user_history_max_threads))
        user_history_messages_per_thread = max(0, min(10000, user_history_messages_per_thread))
        memory_size_limit = max(0, min(100000, memory_size_limit))
    except ValueError:
        user_history_max_threads = 0
        user_history_messages_per_thread = 0
        memory_size_limit = 8000

    config['USER_HISTORY_MAX_THREADS'] = user_history_max_threads
    config['USER_HISTORY_MESSAGES_PER_THREAD'] = user_history_messages_per_thread
    config['MEMORY_SIZE_LIMIT'] = memory_size_limit
    save_config(config)

    return JsonResponse({'success': True})


def wipe_memory(request):
    """Wipe per-persona memory (POST)"""
    if request.method == 'POST':
        config = load_config()
        selected_persona = request.POST.get('persona', config.get("DEFAULT_PERSONA", "assistant"))
        delete_memory(selected_persona)

        if request.headers.get('HX-Request'):
            context = _build_memory_view_context(
                config, selected_persona,
                success="Memory wiped successfully", just_updated=True,
            )
            return render(request, 'memory/memory_main.html', context)

        return redirect(f"{reverse('memory')}?success=Memory wiped successfully")

    return redirect('memory')


def modify_memory(request):
    """Modify memory based on user command (HTMX endpoint)"""
    if request.method != 'POST':
        return HttpResponse(status=405)

    command = request.POST.get('command', '').strip()
    if not command:
        return HttpResponse(status=400)

    config = load_config()
    if not config:
        return HttpResponse("Configuration not found", status=500)

    selected_persona = request.POST.get('persona', config.get("DEFAULT_PERSONA", "assistant"))
    api_key = config.get("OPENROUTER_API_KEY")
    site_url = config.get("SITE_URL")
    site_name = config.get("SITE_NAME")

    persona_dir = str(django_settings.PERSONAS_DIR / selected_persona)
    persona_identity = get_persona_identity(persona_dir)
    memory_model = get_memory_model(config, selected_persona, str(django_settings.PERSONAS_DIR))

    manager = MemoryManager(api_key, memory_model, site_url, site_name)
    updated_memory = manager.modify_memory_with_command(selected_persona, persona_identity, command)

    context = _build_memory_view_context(
        config, selected_persona,
        success="Memory Updated" if updated_memory else None,
        error="Failed to update memory" if not updated_memory else None,
        just_updated=True,
    )
    return render(request, 'memory/memory_main.html', context)


# =============================================================================
# Global Context File Endpoints
# =============================================================================

def upload_context_file(request):
    """Upload a user context file (HTMX/AJAX endpoint)"""
    if request.method != 'POST':
        return HttpResponse(status=405)

    uploaded_file = request.FILES.get('file')
    if not uploaded_file:
        return HttpResponse("No file provided", status=400)

    # Upload the file
    filename = do_upload_context(uploaded_file)

    # For AJAX requests (from modal), return JSON
    if request.headers.get('X-Requested-With') == 'XMLHttpRequest':
        return JsonResponse({
            'success': bool(filename),
            'filename': filename,
            'files': list_context_files()
        })

    # For HTMX requests, return HTML partial
    config = load_config()
    selected_persona = request.POST.get('persona', config.get("DEFAULT_PERSONA", "assistant"))
    context = _build_memory_view_context(
        config, selected_persona,
        success=f"Uploaded {filename}" if filename else None,
        error="Invalid file type. Only .md and .txt files allowed." if not filename else None,
    )
    return render(request, 'memory/memory_main.html', context)


def delete_context_file(request):
    """Delete a user context file (HTMX/AJAX endpoint)"""
    if request.method != 'POST':
        return HttpResponse(status=405)

    filename = request.POST.get('filename', '')
    if not filename:
        return HttpResponse("No filename provided", status=400)

    # Delete the file
    deleted = do_delete_context(filename)

    # For AJAX requests (from modal), return JSON
    if request.headers.get('X-Requested-With') == 'XMLHttpRequest':
        return JsonResponse({
            'success': deleted,
            'filename': filename,
            'files': list_context_files()
        })

    # For HTMX requests, return HTML partial
    config = load_config()
    selected_persona = request.POST.get('persona', config.get("DEFAULT_PERSONA", "assistant"))
    context = _build_memory_view_context(
        config, selected_persona,
        success=f"Deleted {filename}" if deleted else None,
        error=f"File not found: {filename}" if not deleted else None,
    )
    return render(request, 'memory/memory_main.html', context)


def toggle_context_file(request):
    """Toggle enabled status of a user context file (HTMX/AJAX endpoint)"""
    if request.method != 'POST':
        return HttpResponse(status=405)

    filename = request.POST.get('filename', '')
    if not filename:
        return HttpResponse("No filename provided", status=400)

    # Toggle the file
    new_status = do_toggle_context(filename)

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
    selected_persona = request.POST.get('persona', config.get("DEFAULT_PERSONA", "assistant"))
    context = _build_memory_view_context(config, selected_persona)
    return render(request, 'memory/memory_main.html', context)


def get_context_file_content(request):
    """GET endpoint to retrieve context file content for editing"""
    filename = request.GET.get('filename')
    if not filename:
        return JsonResponse({'error': 'No filename provided'}, status=400)

    filename = os.path.basename(filename)
    file_path = get_user_context_dir() / filename
    if not file_path.exists():
        return JsonResponse({'error': 'File not found'}, status=404)

    content = file_path.read_text()
    return JsonResponse({'filename': filename, 'content': content})


def save_context_file_content(request):
    """POST endpoint to save edited context file content"""
    if request.method != 'POST':
        return JsonResponse({'error': 'POST required'}, status=405)

    filename = request.POST.get('filename')
    content = request.POST.get('content', '')

    if not filename:
        return JsonResponse({'error': 'No filename provided'}, status=400)

    filename = os.path.basename(filename)
    file_path = get_user_context_dir() / filename
    if not file_path.exists():
        return JsonResponse({'error': 'File not found'}, status=404)

    file_path.write_text(content)
    return JsonResponse({'success': True, 'filename': filename})


# =============================================================================
# Persona-specific Context File Endpoints
# =============================================================================

def upload_persona_context_file(request):
    """Upload a context file for a specific persona (AJAX endpoint)"""
    if request.method != 'POST':
        return JsonResponse({'error': 'POST required'}, status=405)

    persona = request.POST.get('persona')
    if not persona:
        return JsonResponse({'error': 'No persona specified'}, status=400)

    uploaded_file = request.FILES.get('file')
    if not uploaded_file:
        return JsonResponse({'error': 'No file provided'}, status=400)

    filename = do_upload_persona_context(persona, uploaded_file)
    if not filename:
        return JsonResponse({
            'error': 'Invalid file type. Only .md and .txt files allowed.'
        }, status=400)

    return JsonResponse({
        'success': True,
        'filename': filename,
        'files': list_persona_context_files(persona)
    })


def delete_persona_context_file(request):
    """Delete a context file from a specific persona (AJAX endpoint)"""
    if request.method != 'POST':
        return JsonResponse({'error': 'POST required'}, status=405)

    persona = request.POST.get('persona')
    if not persona:
        return JsonResponse({'error': 'No persona specified'}, status=400)

    filename = request.POST.get('filename')
    if not filename:
        return JsonResponse({'error': 'No filename provided'}, status=400)

    deleted = do_delete_persona_context(persona, filename)
    return JsonResponse({
        'success': deleted,
        'filename': filename,
        'files': list_persona_context_files(persona)
    })


def toggle_persona_context_file(request):
    """Toggle enabled status of a persona's context file (AJAX endpoint)"""
    if request.method != 'POST':
        return JsonResponse({'error': 'POST required'}, status=405)

    persona = request.POST.get('persona')
    if not persona:
        return JsonResponse({'error': 'No persona specified'}, status=400)

    filename = request.POST.get('filename')
    if not filename:
        return JsonResponse({'error': 'No filename provided'}, status=400)

    new_status = do_toggle_persona_context(persona, filename)
    return JsonResponse({
        'success': True,
        'filename': filename,
        'enabled': new_status,
        'files': list_persona_context_files(persona)
    })


def get_persona_context_file_content(request):
    """GET endpoint to retrieve a persona's context file content for editing"""
    persona = request.GET.get('persona')
    if not persona:
        return JsonResponse({'error': 'No persona specified'}, status=400)

    filename = request.GET.get('filename')
    if not filename:
        return JsonResponse({'error': 'No filename provided'}, status=400)

    content = do_get_persona_content(persona, filename)
    if content is None:
        return JsonResponse({'error': 'File not found'}, status=404)

    return JsonResponse({'filename': filename, 'content': content})


def save_persona_context_file_content(request):
    """POST endpoint to save edited persona context file content"""
    if request.method != 'POST':
        return JsonResponse({'error': 'POST required'}, status=405)

    persona = request.POST.get('persona')
    if not persona:
        return JsonResponse({'error': 'No persona specified'}, status=400)

    filename = request.POST.get('filename')
    if not filename:
        return JsonResponse({'error': 'No filename provided'}, status=400)

    content = request.POST.get('content', '')
    saved = do_save_persona_content(persona, filename, content)

    if not saved:
        return JsonResponse({'error': 'File not found'}, status=404)

    return JsonResponse({'success': True, 'filename': filename})


# =============================================================================
# Unified Local Directory Endpoints
# Both global and persona-scoped, distinguished by optional 'persona' param
# =============================================================================

def browse_directories(request):
    """Browse filesystem directories (GET, shared by both global and persona)"""
    path = request.GET.get('path', '')
    show_hidden = request.GET.get('show_hidden') == '1'
    result = browse_directory(path, show_hidden)
    return JsonResponse(result)


def add_local_context_dir(request):
    """Add a local directory to context config (POST)"""
    if request.method != 'POST':
        return JsonResponse({'error': 'POST required'}, status=405)

    dir_path = request.POST.get('dir_path', '').strip()
    if not dir_path:
        return JsonResponse({'error': 'No directory path provided'}, status=400)

    persona = request.POST.get('persona', '').strip()
    if persona:
        success, result, files = add_persona_context_local_directory(persona, dir_path)
        dirs = list_persona_context_local_directories(persona)
    else:
        success, result, files = add_context_local_directory(dir_path)
        dirs = list_context_local_directories()

    if not success:
        return JsonResponse({'error': result}, status=400)

    return JsonResponse({'directories': dirs})


def remove_local_context_dir(request):
    """Remove a local directory from context config (POST)"""
    if request.method != 'POST':
        return JsonResponse({'error': 'POST required'}, status=405)

    dir_path = request.POST.get('dir_path', '').strip()
    if not dir_path:
        return JsonResponse({'error': 'No directory path provided'}, status=400)

    persona = request.POST.get('persona', '').strip()
    if persona:
        remove_persona_context_local_directory(persona, dir_path)
        dirs = list_persona_context_local_directories(persona)
    else:
        remove_context_local_directory(dir_path)
        dirs = list_context_local_directories()

    return JsonResponse({'directories': dirs})


def toggle_local_context_file(request):
    """Toggle a file in a local directory (POST)"""
    if request.method != 'POST':
        return JsonResponse({'error': 'POST required'}, status=405)

    dir_path = request.POST.get('dir_path', '').strip()
    filename = request.POST.get('filename', '').strip()
    if not dir_path or not filename:
        return JsonResponse({'error': 'dir_path and filename required'}, status=400)

    persona = request.POST.get('persona', '').strip()
    if persona:
        toggle_persona_context_local_file(persona, dir_path, filename)
        dirs = list_persona_context_local_directories(persona)
    else:
        toggle_context_local_file(dir_path, filename)
        dirs = list_context_local_directories()

    return JsonResponse({'directories': dirs})


def get_local_context_file_content(request):
    """Read a file from a local directory (GET)"""
    dir_path = request.GET.get('dir_path', '').strip()
    filename = request.GET.get('filename', '').strip()
    if not dir_path or not filename:
        return JsonResponse({'error': 'dir_path and filename required'}, status=400)

    # Security: verify directory is registered in config
    persona = request.GET.get('persona', '').strip()
    if persona:
        dirs = list_persona_context_local_directories(persona)
    else:
        dirs = list_context_local_directories()

    resolved = os.path.realpath(dir_path)
    if not any(d['path'] == resolved for d in dirs):
        return JsonResponse({'error': 'Directory not registered'}, status=403)

    content = get_context_local_file_content(dir_path, filename)
    if content is None:
        return JsonResponse({'error': 'File not found'}, status=404)

    return JsonResponse({'filename': os.path.basename(filename), 'content': content})


def refresh_local_context_dir(request):
    """Refresh files in a local directory (POST)"""
    if request.method != 'POST':
        return JsonResponse({'error': 'POST required'}, status=405)

    dir_path = request.POST.get('dir_path', '').strip()
    if not dir_path:
        return JsonResponse({'error': 'No directory path provided'}, status=400)

    persona = request.POST.get('persona', '').strip()
    if persona:
        refresh_persona_context_local_directory(persona, dir_path)
        dirs = list_persona_context_local_directories(persona)
    else:
        refresh_context_local_directory(dir_path)
        dirs = list_context_local_directories()

    return JsonResponse({'directories': dirs})
