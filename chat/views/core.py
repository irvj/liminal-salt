import json
import logging

from django.shortcuts import render, redirect
from django.conf import settings as django_settings

from ..services import fetch_available_models, validate_api_key, get_providers
from ..utils import (
    load_config, save_config, group_models_by_provider,
    flatten_models_with_provider_prefix,
)

logger = logging.getLogger(__name__)


def _get_theme_list():
    """Helper function to get list of available themes from the themes directory"""
    themes_dir = django_settings.BASE_DIR / 'chat' / 'static' / 'themes'
    themes = []

    if themes_dir.exists():
        for theme_file in sorted(themes_dir.glob('*.json')):
            try:
                with open(theme_file) as f:
                    data = json.load(f)
                    themes.append({
                        'id': data.get('id', theme_file.stem),
                        'name': data.get('name', theme_file.stem.title())
                    })
            except (json.JSONDecodeError, KeyError):
                continue

    return themes


def _get_theme_context(config=None):
    """Helper function to get theme context for templates"""
    if config is None:
        config = load_config()
    return {
        'color_theme': config.get('THEME', 'liminal-salt'),
        'theme_mode': config.get('THEME_MODE', 'dark')
    }

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
                "DEFAULT_PERSONA": "assistant",
                "PERSONAS_DIR": "personas",
                "CONTEXT_HISTORY_LIMIT": 50,
                "SESSIONS_DIR": "sessions"
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
            selected_theme = request.POST.get('theme', 'liminal-salt').strip()
            selected_mode = request.POST.get('theme_mode', 'dark').strip()

            if not selected_model:
                # Re-fetch models and themes for error display
                models = fetch_available_models(api_key)
                if models:
                    grouped_models = group_models_by_provider(models)
                    model_options = flatten_models_with_provider_prefix(grouped_models)
                    themes = _get_theme_list()
                    return render(request, 'setup/step2.html', {
                        'error': 'Please select a model',
                        'model_count': len(models),
                        'model_options': model_options,
                        'selected_model': selected_model,
                        'themes': themes,
                        'themes_json': json.dumps(themes),
                        'selected_theme': selected_theme,
                        'selected_mode': selected_mode
                    })
                else:
                    # API key is no longer valid, go back to step 1
                    logger.error("Failed to re-fetch models in step 2")
                    request.session['setup_step'] = 1
                    request.session.modified = True
                    return redirect('setup')

            # Update config.json with selected model and theme
            config['MODEL'] = selected_model
            config['THEME'] = selected_theme
            config['THEME_MODE'] = selected_mode
            save_config(config)
            logger.info(f"Setup complete: model {selected_model}, theme {selected_theme} ({selected_mode}) saved")

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
        themes = _get_theme_list()

        return render(request, 'setup/step2.html', {
            'model_count': len(models),
            'model_options': model_options,
            'themes': themes,
            'themes_json': json.dumps(themes),
            'selected_theme': 'liminal-salt',
            'selected_mode': 'dark'
        })
