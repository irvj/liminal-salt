import json
import logging

from django.shortcuts import render, redirect
from django.http import HttpResponse, JsonResponse
from django.conf import settings as django_settings

from ..services import (
    fetch_available_models, get_providers,
    get_available_personas, get_persona_model, get_persona_config,
    save_persona_config,
    list_persona_context_files,
    list_persona_context_local_directories,
)
from ..services.persona_manager import (
    get_persona_preview, save_persona_identity,
    create_persona as create_persona_dir,
    delete_persona as delete_persona_dir,
    rename_persona, persona_exists,
)
from ..services.session_manager import update_persona_across_sessions
from ..utils import (
    load_config, save_config, group_models_by_provider,
    flatten_models_with_provider_prefix,
)

logger = logging.getLogger(__name__)


def _persona_context_badge_count(persona_name):
    """Count enabled uploaded files + enabled local directory files for a persona."""
    files = list_persona_context_files(persona_name)
    enabled_uploaded = sum(1 for f in files if f.get('enabled'))
    local_dirs = list_persona_context_local_directories(persona_name)
    enabled_local = sum(
        1 for d in local_dirs for f in d.get('files', []) if f.get('enabled')
    )
    return enabled_uploaded + enabled_local


def _persona_context_extras(persona_name):
    """Return extra context dict keys for local directory support."""
    local_dirs = list_persona_context_local_directories(persona_name)
    return {
        'persona_context_local_dirs_json': json.dumps(local_dirs),
        'persona_context_badge_count': _persona_context_badge_count(persona_name),
    }


def _fetch_available_models_list(config):
    """Fetch and format available models if API key exists. Returns (has_api_key, models_list)."""
    provider = config.get("PROVIDER", "openrouter")
    api_key = None
    if provider == 'openrouter':
        api_key = config.get("OPENROUTER_API_KEY")

    if not api_key:
        return False, []

    models_list = fetch_available_models(api_key)
    if not models_list:
        return True, []

    grouped = group_models_by_provider(models_list)
    model_options = flatten_models_with_provider_prefix(grouped)
    return True, [{'id': m[0], 'display': m[1]} for m in model_options]


def persona_settings(request):
    """Persona settings view"""
    config = load_config()
    if not config:
        return redirect('setup')

    personas_dir = str(django_settings.PERSONAS_DIR)
    available_personas = get_available_personas(personas_dir)
    default_persona = config.get("DEFAULT_PERSONA", "")
    model = config.get("MODEL", "")

    # Read persona preview
    persona_preview = ""
    selected_persona = default_persona
    persona_model = None
    if available_personas:
        selected_persona = request.GET.get('persona', request.GET.get('preview', default_persona))
        if selected_persona:
            persona_preview = get_persona_preview(selected_persona)
            persona_model = get_persona_model(selected_persona, personas_dir)

    # Get persona-specific context files
    persona_context_files = []
    if selected_persona:
        persona_context_files = list_persona_context_files(selected_persona)

    context = {
        'model': model,
        'personas': available_personas,
        'default_persona': default_persona,
        'selected_persona': selected_persona,
        'persona_preview': persona_preview,
        'persona_model': persona_model or '',
        'persona_context_files': persona_context_files,
        'persona_context_files_json': json.dumps(persona_context_files),
        'success': request.GET.get('success'),
        **_persona_context_extras(selected_persona),
    }

    if request.headers.get('HX-Request'):
        return render(request, 'persona/persona_main.html', context)

    return redirect('chat')


def save_persona_file(request):
    """Save edited persona file content and optionally rename persona"""
    if request.method != 'POST':
        return HttpResponse(status=405)

    persona = request.POST.get('persona', '').strip()
    new_name = request.POST.get('new_name', '').strip()
    content = request.POST.get('content', '')

    if not persona:
        return HttpResponse("Persona name required", status=400)

    config = load_config()
    personas_dir = str(django_settings.PERSONAS_DIR)

    is_rename = new_name and new_name != persona

    if is_rename:
        success, error = rename_persona(old_name=persona, new_name=new_name,
                                        config=config, save_config_fn=save_config)
        if not success:
            return HttpResponse(error, status=400)
        final_persona = new_name
    else:
        final_persona = persona

    # Write identity content
    save_persona_identity(final_persona, content)

    # Reload config in case it was updated by rename
    config = load_config()

    # Return updated settings partial
    available_personas = get_available_personas(personas_dir)
    default_persona = config.get("DEFAULT_PERSONA", "")
    model = config.get("MODEL", "")
    provider = config.get("PROVIDER", "openrouter")
    providers = get_providers()

    has_api_key, available_models = _fetch_available_models_list(config)
    persona_model = get_persona_model(final_persona, personas_dir)

    context = {
        'model': model,
        'provider': provider,
        'providers': providers,
        'providers_json': json.dumps(providers),
        'has_api_key': has_api_key,
        'personas': available_personas,
        'default_persona': default_persona,
        'selected_persona': final_persona,
        'persona_preview': content,
        'persona_model': persona_model or '',
        'available_models': available_models,
        'available_models_json': json.dumps(available_models),
        'persona_context_files': list_persona_context_files(final_persona),
        'persona_context_files_json': json.dumps(list_persona_context_files(final_persona)),
        'success': "Persona saved" + (" and renamed" if is_rename else ""),
        **_persona_context_extras(final_persona),
    }
    return render(request, 'persona/persona_main.html', context)


def create_persona(request):
    """Create a new persona"""
    if request.method != 'POST':
        return HttpResponse(status=405)

    name = request.POST.get('name', '').strip()
    content = request.POST.get('content', '')

    if not name:
        return HttpResponse("Personality name required", status=400)

    success, error = create_persona_dir(name, content)
    if not success:
        return HttpResponse(error, status=400)

    config = load_config()
    personas_dir = str(django_settings.PERSONAS_DIR)

    available_personas = get_available_personas(personas_dir)
    default_persona = config.get("DEFAULT_PERSONA", "")
    model = config.get("MODEL", "")
    provider = config.get("PROVIDER", "openrouter")
    providers = get_providers()

    has_api_key, available_models = _fetch_available_models_list(config)

    context = {
        'model': model,
        'provider': provider,
        'providers': providers,
        'providers_json': json.dumps(providers),
        'has_api_key': has_api_key,
        'personas': available_personas,
        'default_persona': default_persona,
        'selected_persona': name,
        'persona_preview': content,
        'persona_model': '',
        'available_models': available_models,
        'available_models_json': json.dumps(available_models),
        'persona_context_files': [],
        'persona_context_files_json': '[]',
        'success': "Persona created",
        **_persona_context_extras(name),
    }
    return render(request, 'persona/persona_main.html', context)


def delete_persona(request):
    """Delete a persona"""
    if request.method != 'POST':
        return HttpResponse(status=405)

    persona = request.POST.get('persona', '').strip()

    if not persona:
        return HttpResponse("Persona name required", status=400)

    if not persona_exists(persona):
        return HttpResponse("Persona not found", status=404)

    config = load_config()
    personas_dir = str(django_settings.PERSONAS_DIR)

    available_personas = get_available_personas(personas_dir)
    if len(available_personas) <= 1:
        return HttpResponse("Cannot delete the only persona", status=400)

    # Delete persona and all associated data (dir, memory, context files)
    delete_persona_dir(persona)

    # Update config if this was the default persona
    default_persona = config.get("DEFAULT_PERSONA", "")
    if default_persona == persona:
        available_personas = get_available_personas(personas_dir)
        if available_personas:
            config["DEFAULT_PERSONA"] = available_personas[0]
            save_config(config)
            default_persona = available_personas[0]

    # Update sessions that used this persona to use the default
    update_persona_across_sessions(persona, default_persona)

    # Build response
    available_personas = get_available_personas(personas_dir)
    model = config.get("MODEL", "")
    provider = config.get("PROVIDER", "openrouter")
    providers = get_providers()

    has_api_key, available_models = _fetch_available_models_list(config)
    persona_preview = get_persona_preview(default_persona)
    persona_model = get_persona_model(default_persona, personas_dir)

    context = {
        'model': model,
        'provider': provider,
        'providers': providers,
        'providers_json': json.dumps(providers),
        'has_api_key': has_api_key,
        'personas': available_personas,
        'default_persona': default_persona,
        'selected_persona': default_persona,
        'persona_preview': persona_preview,
        'persona_model': persona_model or '',
        'available_models': available_models,
        'available_models_json': json.dumps(available_models),
        'persona_context_files': list_persona_context_files(default_persona),
        'persona_context_files_json': json.dumps(list_persona_context_files(default_persona)),
        'success': "Persona deleted",
        **_persona_context_extras(default_persona),
    }
    return render(request, 'persona/persona_main.html', context)


def save_persona_model(request):
    """Save model override for a persona (POST)"""
    if request.method != 'POST':
        return JsonResponse({'error': 'Method not allowed'}, status=405)

    persona = request.POST.get('persona', '').strip()
    model = request.POST.get('model', '').strip()

    if not persona:
        return JsonResponse({'error': 'Persona is required'}, status=400)

    if not persona_exists(persona):
        return JsonResponse({'error': 'Persona not found'}, status=404)

    # Load existing config or create new
    personas_dir = str(django_settings.PERSONAS_DIR)
    config = get_persona_config(persona, personas_dir)

    if model:
        config["model"] = model
    elif "model" in config:
        del config["model"]

    save_persona_config(persona, config, personas_dir)

    return JsonResponse({'success': True, 'model': model or None})
