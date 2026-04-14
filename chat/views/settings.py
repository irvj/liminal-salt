import json
import logging
import os

from django.shortcuts import render, redirect
from django.http import HttpResponse, JsonResponse
from django.conf import settings as django_settings

from ..services import (
    validate_api_key, get_providers,
    get_available_personas, get_persona_model, list_persona_context_files,
    list_context_files,
    upload_context_file as do_upload_context,
    delete_context_file as do_delete_context,
    toggle_context_file as do_toggle_context,
    get_context_file_content as do_get_context_content,
    save_context_file_content as do_save_context_content,
    list_context_local_directories,
)
from ..services.persona_manager import get_persona_preview
from ..utils import load_config, save_config, get_formatted_model_list

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


def settings(request):
    """Settings view"""
    config = load_config()
    if not config:
        return redirect('setup')

    model = config.get("MODEL", "")
    provider = config.get("PROVIDER", "openrouter")
    providers = get_providers()

    # Check if API key exists for current provider
    has_api_key = False
    if provider == 'openrouter':
        api_key = config.get("OPENROUTER_API_KEY")
        has_api_key = bool(api_key)

    # Global context files
    ctx_files = list_context_files()
    local_dirs = list_context_local_directories()

    context = {
        'model': model,
        'provider': provider,
        'providers': providers,
        'providers_json': json.dumps(providers),
        'has_api_key': has_api_key,
        'context_history_limit': config.get('CONTEXT_HISTORY_LIMIT', 50),
        'context_files': ctx_files,
        'context_local_dirs_json': json.dumps(local_dirs),
        'context_badge_count': _context_badge_count(),
        'success': request.GET.get('success'),
    }

    # Return partial for HTMX requests, redirect others to chat
    if request.headers.get('HX-Request'):
        return render(request, 'settings/settings_main.html', context)

    return redirect('chat')


def save_settings(request):
    """Save settings (POST) - handles saving default persona"""
    if request.method == 'POST':
        selected_persona = request.POST.get('persona', '').strip()
        redirect_to = request.POST.get('redirect_to', 'settings')
        config = load_config()
        success_msg = None

        # Personality is required - fall back to "assistant" if empty
        if not selected_persona:
            selected_persona = "assistant"

        # Update if different from current
        if selected_persona != config.get("DEFAULT_PERSONA", ""):
            config["DEFAULT_PERSONA"] = selected_persona
            save_config(config)
            success_msg = "Default persona updated"

        # For HTMX requests, return the appropriate partial
        if request.headers.get('HX-Request'):
            personas_dir = str(django_settings.PERSONAS_DIR)
            available_personas = get_available_personas(personas_dir)
            default_persona = config.get("DEFAULT_PERSONA", "")
            model = config.get("MODEL", "")
            provider = config.get("PROVIDER", "openrouter")

            # Fetch models
            api_key = config.get("OPENROUTER_API_KEY", "")
            has_api_key = bool(api_key)
            available_models = get_formatted_model_list(api_key)

            # Read persona preview for the newly set default (if set)
            persona_preview = get_persona_preview(default_persona) if default_persona else ""
            persona_model = get_persona_model(default_persona, personas_dir) if default_persona else None

            # Return persona page if redirecting there
            if redirect_to == 'persona':
                persona_context_files = list_persona_context_files(default_persona) if default_persona else []
                context = {
                    'model': model,
                    'personas': available_personas,
                    'default_persona': default_persona,
                    'selected_persona': default_persona,
                    'persona_preview': persona_preview,
                    'persona_model': persona_model or '',
                    'persona_context_files': persona_context_files,
                    'persona_context_files_json': json.dumps(persona_context_files),
                    'available_models': available_models,
                    'available_models_json': json.dumps(available_models),
                    'success': success_msg,
                }
                return render(request, 'persona/persona_main.html', context)

            # Otherwise return settings page
            providers = get_providers()
            context = {
                'model': model,
                'provider': provider,
                'providers': providers,
                'providers_json': json.dumps(providers),
                'has_api_key': has_api_key,
                'success': success_msg,
            }
            return render(request, 'settings/settings_main.html', context)

        # Non-HTMX redirect
        redirect_url = 'persona_settings' if redirect_to == 'persona' else 'settings'
        if success_msg:
            return redirect(redirect_url + '?success=' + success_msg)
        return redirect(redirect_url)

    return redirect('settings')


def save_context_history_limit(request):
    """Save CONTEXT_HISTORY_LIMIT setting (AJAX endpoint)"""
    if request.method != 'POST':
        return HttpResponse(status=405)

    context_history_limit = request.POST.get('context_history_limit', 50)
    try:
        context_history_limit = int(context_history_limit)
        context_history_limit = max(10, min(500, context_history_limit))  # Clamp between 10-500
    except ValueError:
        context_history_limit = 50

    config = load_config()
    config['CONTEXT_HISTORY_LIMIT'] = context_history_limit
    save_config(config)

    return JsonResponse({'success': True, 'context_history_limit': context_history_limit})


def validate_provider_api_key(request):
    """Validate API key and return models list (JSON endpoint for Settings page)"""
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
        available_models = get_formatted_model_list(api_key)
        if not available_models:
            return JsonResponse({'valid': False, 'error': 'Could not fetch models'})

        return JsonResponse({
            'valid': True,
            'models': available_models
        })

    return JsonResponse({'valid': False, 'error': 'Unknown provider'})


def save_provider_model(request):
    """Save provider and model settings (JSON endpoint for Settings page)"""
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


# =============================================================================
# Global Context File Endpoints
# =============================================================================

def upload_context_file(request):
    """Upload a user context file (AJAX endpoint)"""
    if request.method != 'POST':
        return HttpResponse(status=405)

    uploaded_file = request.FILES.get('file')
    if not uploaded_file:
        return JsonResponse({'error': 'No file provided'}, status=400)

    filename = do_upload_context(uploaded_file)
    if not filename:
        return JsonResponse({
            'error': 'Invalid file type. Only .md and .txt files allowed.'
        }, status=400)

    return JsonResponse({
        'success': True,
        'filename': filename,
        'files': list_context_files()
    })


def delete_context_file(request):
    """Delete a user context file (AJAX endpoint)"""
    if request.method != 'POST':
        return HttpResponse(status=405)

    filename = request.POST.get('filename', '')
    if not filename:
        return JsonResponse({'error': 'No filename provided'}, status=400)

    deleted = do_delete_context(filename)
    return JsonResponse({
        'success': deleted,
        'filename': filename,
        'files': list_context_files()
    })


def toggle_context_file(request):
    """Toggle enabled status of a user context file (AJAX endpoint)"""
    if request.method != 'POST':
        return HttpResponse(status=405)

    filename = request.POST.get('filename', '')
    if not filename:
        return JsonResponse({'error': 'No filename provided'}, status=400)

    new_status = do_toggle_context(filename)
    return JsonResponse({
        'success': True,
        'filename': filename,
        'enabled': new_status,
        'files': list_context_files()
    })


def get_context_file_content(request):
    """GET endpoint to retrieve context file content for editing"""
    filename = request.GET.get('filename')
    if not filename:
        return JsonResponse({'error': 'No filename provided'}, status=400)

    content = do_get_context_content(filename)
    if content is None:
        return JsonResponse({'error': 'File not found'}, status=404)

    return JsonResponse({'filename': os.path.basename(filename), 'content': content})


def save_context_file_content(request):
    """POST endpoint to save edited context file content"""
    if request.method != 'POST':
        return JsonResponse({'error': 'POST required'}, status=405)

    filename = request.POST.get('filename')
    content = request.POST.get('content', '')

    if not filename:
        return JsonResponse({'error': 'No filename provided'}, status=400)

    if not do_save_context_content(filename, content):
        return JsonResponse({'error': 'File not found'}, status=404)

    return JsonResponse({'success': True, 'filename': os.path.basename(filename)})
