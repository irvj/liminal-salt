import json
import logging
import os
from datetime import datetime

from django.shortcuts import redirect, render
from django.http import HttpResponse, JsonResponse
from django.conf import settings as django_settings
from django.urls import reverse

from ..services import (
    Summarizer,
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


def memory(request):
    """User memory view"""
    config = load_config()
    if not config:
        return redirect('setup')

    ltm_file = django_settings.LTM_FILE
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
        'user_history_max_threads': config.get('USER_HISTORY_MAX_THREADS', 10),
        'user_history_messages_per_thread': config.get('USER_HISTORY_MESSAGES_PER_THREAD', 100),
        **_memory_context(context_files),
    }

    # Return partial for HTMX requests, redirect others to chat
    if request.headers.get('HX-Request'):
        return render(request, 'memory/memory_main.html', context)

    return redirect('chat')


def update_memory(request):
    """Update long-term memory (POST)"""
    if request.method == 'POST':
        config = load_config()
        ltm_file = django_settings.LTM_FILE
        api_key = config.get("OPENROUTER_API_KEY")
        model = config.get("MODEL")
        site_url = config.get("SITE_URL")
        site_name = config.get("SITE_NAME")

        success_msg = None
        error_msg = None

        try:
            # Get memory generation limits from config
            user_history_max_threads = config.get('USER_HISTORY_MAX_THREADS', 10)
            user_history_messages_per_thread = config.get('USER_HISTORY_MESSAGES_PER_THREAD', 100)

            # Aggregate threads from sessions with limits
            threads = aggregate_all_sessions_messages(
                user_history_max_threads=user_history_max_threads if user_history_max_threads > 0 else None,
                user_history_messages_per_thread=user_history_messages_per_thread if user_history_messages_per_thread > 0 else None
            )

            if not threads:
                error_msg = "No threads found in any session"
            else:
                # Update memory
                summarizer = Summarizer(api_key, model, site_url, site_name)
                summarizer.update_long_term_memory(threads, str(ltm_file))
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

            ctx_files = list_context_files()
            context = {
                'model': model,
                'memory_content': memory_content,
                'last_update': last_update,
                'success': success_msg,
                'error': error_msg,
                'just_updated': True if success_msg else False,
                'context_files': ctx_files,
                'user_history_max_threads': config.get('USER_HISTORY_MAX_THREADS', 10),
                'user_history_messages_per_thread': config.get('USER_HISTORY_MESSAGES_PER_THREAD', 100),
                **_memory_context(ctx_files),
            }
            return render(request, 'memory/memory_main.html', context)

        # For regular requests, redirect with query params
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

    try:
        user_history_max_threads = int(user_history_max_threads)
        user_history_messages_per_thread = int(user_history_messages_per_thread)
        # Clamp to reasonable values (0 = unlimited)
        user_history_max_threads = max(0, min(100, user_history_max_threads))
        user_history_messages_per_thread = max(0, min(10000, user_history_messages_per_thread))
    except ValueError:
        user_history_max_threads = 0
        user_history_messages_per_thread = 0

    config['USER_HISTORY_MAX_THREADS'] = user_history_max_threads
    config['USER_HISTORY_MESSAGES_PER_THREAD'] = user_history_messages_per_thread
    save_config(config)

    return JsonResponse({'success': True})


def wipe_memory(request):
    """Wipe long-term memory (POST)"""
    if request.method == 'POST':
        config = load_config()
        ltm_file = django_settings.LTM_FILE
        if os.path.exists(ltm_file):
            os.remove(ltm_file)

        # For HTMX requests, return the partial directly
        if request.headers.get('HX-Request'):
            ctx_files = list_context_files()
            context = {
                'model': config.get("MODEL", ""),
                'memory_content': "",
                'last_update': None,
                'success': "Memory wiped successfully",
                'error': None,
                'just_updated': True,
                'context_files': ctx_files,
                'user_history_max_threads': config.get('USER_HISTORY_MAX_THREADS', 10),
                'user_history_messages_per_thread': config.get('USER_HISTORY_MESSAGES_PER_THREAD', 100),
                **_memory_context(ctx_files),
            }
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

    api_key = config.get("OPENROUTER_API_KEY")
    model = config.get("MODEL")
    site_url = config.get("SITE_URL")
    site_name = config.get("SITE_NAME")
    ltm_file = django_settings.LTM_FILE

    # Call the summarizer to modify memory
    summarizer = Summarizer(api_key, model, site_url, site_name)
    updated_memory = summarizer.modify_memory_with_command(command, str(ltm_file))

    # Get last update time
    last_update = None
    if os.path.exists(ltm_file):
        last_update = datetime.fromtimestamp(os.path.getmtime(ltm_file))

    # Return the updated memory view
    ctx_files = list_context_files()
    context = {
        'model': model,
        'memory_content': updated_memory if updated_memory else "",
        'last_update': last_update,
        'success': "Memory Updated" if updated_memory else None,
        'error': "Failed to update memory" if not updated_memory else None,
        'just_updated': True,
        'context_files': ctx_files,
        'user_history_max_threads': config.get('USER_HISTORY_MAX_THREADS', 10),
        'user_history_messages_per_thread': config.get('USER_HISTORY_MESSAGES_PER_THREAD', 100),
        **_memory_context(ctx_files),
    }
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
    ltm_file = django_settings.LTM_FILE
    model = config.get("MODEL", "") if config else ""

    last_update = None
    if os.path.exists(ltm_file):
        last_update = datetime.fromtimestamp(os.path.getmtime(ltm_file))

    memory_content = ""
    if os.path.exists(ltm_file):
        with open(ltm_file, 'r') as f:
            memory_content = f.read()

    ctx_files = list_context_files()
    context = {
        'model': model,
        'memory_content': memory_content,
        'last_update': last_update,
        'context_files': ctx_files,
        'success': f"Uploaded {filename}" if filename else None,
        'error': "Invalid file type. Only .md and .txt files allowed." if not filename else None,
        **_memory_context(ctx_files),
    }
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
    ltm_file = django_settings.LTM_FILE
    model = config.get("MODEL", "") if config else ""

    last_update = None
    if os.path.exists(ltm_file):
        last_update = datetime.fromtimestamp(os.path.getmtime(ltm_file))

    memory_content = ""
    if os.path.exists(ltm_file):
        with open(ltm_file, 'r') as f:
            memory_content = f.read()

    ctx_files = list_context_files()
    context = {
        'model': model,
        'memory_content': memory_content,
        'last_update': last_update,
        'context_files': ctx_files,
        'success': f"Deleted {filename}" if deleted else None,
        'error': f"File not found: {filename}" if not deleted else None,
        **_memory_context(ctx_files),
    }
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
    ltm_file = django_settings.LTM_FILE
    model = config.get("MODEL", "") if config else ""

    last_update = None
    if os.path.exists(ltm_file):
        last_update = datetime.fromtimestamp(os.path.getmtime(ltm_file))

    memory_content = ""
    if os.path.exists(ltm_file):
        with open(ltm_file, 'r') as f:
            memory_content = f.read()

    ctx_files = list_context_files()
    context = {
        'model': model,
        'memory_content': memory_content,
        'last_update': last_update,
        'context_files': ctx_files,
        **_memory_context(ctx_files),
    }
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
