import json
import logging
import os
import shutil

from django.shortcuts import render, redirect
from django.http import HttpResponse, JsonResponse
from django.conf import settings as django_settings

from ..services import (
    fetch_available_models, get_providers,
    get_available_personas, get_persona_model, get_persona_config,
    list_persona_context_files,
    list_persona_context_local_directories,
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



def persona_settings(request):
    """Persona settings view"""
    config = load_config()
    if not config:
        return redirect('setup')

    personas_dir = str(django_settings.PERSONAS_DIR)
    available_personas = get_available_personas(personas_dir)
    default_persona = config.get("DEFAULT_PERSONA", "")
    model = config.get("MODEL", "")
    provider = config.get("PROVIDER", "openrouter")

    # Read first persona file preview
    persona_preview = ""
    selected_persona = default_persona
    persona_model = None
    if available_personas:
        selected_persona = request.GET.get('persona', request.GET.get('preview', default_persona))
        # Only load preview if a persona is actually selected
        if selected_persona:
            persona_path = os.path.join(personas_dir, selected_persona)
            if os.path.exists(persona_path):
                md_files = [f for f in os.listdir(persona_path) if f.endswith(".md")]
                if md_files:
                    with open(os.path.join(persona_path, md_files[0]), 'r') as f:
                        content = f.read()
                        persona_preview = content
            # Get persona-specific model if set
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

    # Return partial for HTMX requests, redirect others to chat
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
    old_path = os.path.join(personas_dir, persona)

    # Determine if we're renaming
    is_rename = new_name and new_name != persona

    if is_rename:
        new_path = os.path.join(personas_dir, new_name)

        # Validate new name (only alphanumeric and underscores)
        if not all(c.isalnum() or c == '_' for c in new_name):
            return HttpResponse("Invalid persona name. Use only letters, numbers, and underscores.", status=400)

        # Check if new name already exists
        if os.path.exists(new_path):
            return HttpResponse(f"A persona named '{new_name}' already exists.", status=400)

        # Rename the folder
        if os.path.exists(old_path):
            shutil.move(old_path, new_path)

            # Rename memory file if it exists
            from ..services import rename_memory
            rename_memory(persona, new_name)

            # Update all session files that reference the old persona
            update_persona_across_sessions(persona, new_name)

            # Update config.json if DEFAULT_PERSONA matches old name
            if config.get("DEFAULT_PERSONA") == persona:
                config["DEFAULT_PERSONA"] = new_name
                save_config(config)

            # Use new path for writing content
            persona_path = new_path
            final_persona = new_name
        else:
            return HttpResponse("Original persona not found", status=404)
    else:
        persona_path = old_path
        final_persona = persona

    # Write content to file
    if os.path.exists(persona_path):
        md_files = [f for f in os.listdir(persona_path) if f.endswith(".md")]
        if md_files:
            filepath = os.path.join(persona_path, md_files[0])
            with open(filepath, 'w') as f:
                f.write(content)

    # Reload config in case it was updated
    config = load_config()

    # Return updated settings partial
    available_personas = get_available_personas(personas_dir)
    default_persona = config.get("DEFAULT_PERSONA", "")
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

    # Get persona model
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

    # Validate name (only alphanumeric and underscores)
    if not all(c.isalnum() or c == '_' for c in name):
        return HttpResponse("Invalid persona name. Use only letters, numbers, and underscores.", status=400)

    config = load_config()
    personas_dir = str(django_settings.PERSONAS_DIR)
    persona_path = os.path.join(personas_dir, name)

    # Check if already exists
    if os.path.exists(persona_path):
        return HttpResponse(f"A persona named '{name}' already exists.", status=400)

    # Create the folder and identity.md file
    os.makedirs(persona_path)
    filepath = os.path.join(persona_path, 'identity.md')
    with open(filepath, 'w') as f:
        f.write(content)

    # Return updated settings partial with new persona selected
    available_personas = get_available_personas(personas_dir)
    default_persona = config.get("DEFAULT_PERSONA", "")
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
        'personas': available_personas,
        'default_persona': default_persona,
        'selected_persona': name,
        'persona_preview': content,
        'persona_model': '',  # New persona has no model override
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

    config = load_config()
    personas_dir = str(django_settings.PERSONAS_DIR)
    persona_path = os.path.join(personas_dir, persona)

    # Check if persona exists
    if not os.path.exists(persona_path):
        return HttpResponse("Persona not found", status=404)

    # Get available personas
    available_personas = get_available_personas(personas_dir)

    # Can't delete if it's the only persona
    if len(available_personas) <= 1:
        return HttpResponse("Cannot delete the only persona", status=400)

    # Delete the folder and memory file
    shutil.rmtree(persona_path)
    from ..services import delete_memory
    delete_memory(persona)

    # Update config if this was the default persona
    default_persona = config.get("DEFAULT_PERSONA", "")
    if default_persona == persona:
        # Set a new default
        available_personas = get_available_personas(personas_dir)
        if available_personas:
            config["DEFAULT_PERSONA"] = available_personas[0]
            save_config(config)
            default_persona = available_personas[0]

    # Update sessions that used this persona to use the default
    _update_sessions_persona(persona, default_persona)

    # Reload available personalities after deletion
    available_personas = get_available_personas(personas_dir)
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

    # Read preview for default persona
    persona_preview = ""
    preview_path = os.path.join(personas_dir, default_persona)
    if os.path.exists(preview_path):
        md_files = [f for f in os.listdir(preview_path) if f.endswith(".md")]
        if md_files:
            with open(os.path.join(preview_path, md_files[0]), 'r') as f:
                persona_preview = f.read()

    # Get persona model for the new default
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

    # Validate persona exists
    persona_path = django_settings.PERSONAS_DIR / persona
    if not persona_path.exists():
        return JsonResponse({'error': 'Persona not found'}, status=404)

    # Load existing config or create new
    config_path = persona_path / "config.json"
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
