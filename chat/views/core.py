import json
import logging

from django.shortcuts import render, redirect
from django.conf import settings as django_settings
from django.views.decorators.http import require_http_methods

from ..services import (
    validate_api_key, get_providers, is_app_ready,
    CURRENT_AGREEMENT_VERSION, AGREEMENT_BODY,
)
from ..utils import (
    load_config, save_config, get_formatted_model_list, get_theme_list,
)

logger = logging.getLogger(__name__)


def _get_theme_context(config=None):
    """Helper function to get theme context for templates"""
    if config is None:
        config = load_config()
    return {
        'color_theme': config.get('THEME', 'liminal-salt'),
        'theme_mode': config.get('THEME_MODE', 'dark')
    }


def index(request):
    """Main entry point — redirects to chat or setup based on the gate flags."""
    config = load_config()
    if not is_app_ready(config):
        return redirect('setup')
    return redirect('chat')


def _initial_setup_step(config):
    """
    Starting step for the wizard based on current config state.
    - Setup previously complete (agreement version drifted/wiped) → step 3.
    - Pre-flag install with working key+model (beta upgrade path)   → step 3.
    - Otherwise fresh install — start at step 1.
    """
    config = config or {}
    if config.get("SETUP_COMPLETE"):
        return 3
    if config.get("OPENROUTER_API_KEY") and config.get("MODEL"):
        return 3
    return 1


@require_http_methods(["GET", "POST"])
def setup_wizard(request):
    """First-time setup wizard. Three steps: provider + API key, theme +
    model, agreement. On step 3 completion, writes SETUP_COMPLETE=True and
    AGREEMENT_ACCEPTED=<current version>."""
    config = load_config()
    if is_app_ready(config):
        return redirect('index')

    if 'setup_step' not in request.session:
        request.session['setup_step'] = _initial_setup_step(config)
        request.session.modified = True

    step = request.session.get('setup_step', 1)

    # Back action — any step can post setup_action=back to decrement.
    if request.method == 'POST' and request.POST.get('setup_action') == 'back':
        if step > 1:
            request.session['setup_step'] = step - 1
            request.session.modified = True
        return redirect('setup')

    if step == 1:
        return _setup_step1(request)
    if step == 2:
        return _setup_step2(request)
    if step == 3:
        return _setup_step3(request, config)

    # Bogus session value — reset.
    request.session['setup_step'] = 1
    request.session.modified = True
    return redirect('setup')


def _setup_step1(request):
    """Step 1 — provider selection + API key validation."""
    providers = get_providers()
    config = load_config() or {}

    if request.method == 'POST':
        provider = request.POST.get('provider', 'openrouter')
        api_key = request.POST.get('api_key', '').strip()

        if not api_key:
            return render(request, 'setup/step1.html', {
                'error': 'Please enter an API key',
                'providers': providers,
                'selected_provider': provider,
            })

        if provider == 'openrouter':
            if not validate_api_key(api_key):
                logger.error("API key validation failed")
                return render(request, 'setup/step1.html', {
                    'error': 'Invalid API key. Please check your key and try again.',
                    'api_key': api_key,
                    'providers': providers,
                    'selected_provider': provider,
                })

        logger.info(f"API key validated successfully for provider: {provider}")

        # Preserve any keys already in config (from a prior partial run or
        # beta upgrade) instead of clobbering them with a fresh partial.
        config['PROVIDER'] = provider
        if provider == 'openrouter':
            config['OPENROUTER_API_KEY'] = api_key
        config.setdefault('SITE_URL', 'https://liminalsalt.app')
        config.setdefault('SITE_NAME', 'Liminal Salt')
        config.setdefault('DEFAULT_PERSONA', 'assistant')
        config.setdefault('CONTEXT_HISTORY_LIMIT', 50)
        config.setdefault('MODEL', '')
        save_config(config)

        request.session['setup_step'] = 2
        request.session.modified = True
        return redirect('setup')

    return render(request, 'setup/step1.html', {
        'providers': providers,
        'selected_provider': config.get('PROVIDER', 'openrouter'),
        'api_key': config.get('OPENROUTER_API_KEY', ''),
    })


def _setup_step2(request):
    """Step 2 — theme + model selection."""
    config = load_config() or {}
    api_key = config.get('OPENROUTER_API_KEY')

    if not api_key:
        logger.error("No API key found in config.json at step 2")
        request.session['setup_step'] = 1
        request.session.modified = True
        return redirect('setup')

    if request.method == 'POST':
        selected_model = request.POST.get('model', '').strip()
        selected_theme = request.POST.get('theme', 'liminal-salt').strip()
        selected_mode = request.POST.get('theme_mode', 'dark').strip()

        if not selected_model:
            available_models = get_formatted_model_list(api_key)
            if not available_models:
                # Key went bad between steps — surface it, don't silent-bounce.
                return render(request, 'setup/step2.html', {
                    'error': 'Could not fetch models from OpenRouter. Go back and re-enter your API key.',
                    'available_models': [],
                    'available_models_json': '[]',
                    'themes': get_theme_list(),
                    'themes_json': json.dumps(get_theme_list()),
                    'selected_theme': selected_theme,
                    'selected_mode': selected_mode,
                })
            themes = get_theme_list()
            return render(request, 'setup/step2.html', {
                'error': 'Please select a model',
                'model_count': len(available_models),
                'available_models': available_models,
                'available_models_json': json.dumps(available_models),
                'selected_model': selected_model,
                'themes': themes,
                'themes_json': json.dumps(themes),
                'selected_theme': selected_theme,
                'selected_mode': selected_mode,
            })

        config['MODEL'] = selected_model
        config['THEME'] = selected_theme
        config['THEME_MODE'] = selected_mode
        save_config(config)
        logger.info(f"Step 2 complete: model {selected_model}, theme {selected_theme} ({selected_mode})")

        request.session['setup_step'] = 3
        request.session.modified = True
        return redirect('setup')

    # GET — fetch models for the dropdown. Failure here used to silently
    # bounce back to step 1; now surface it as an inline error so the user
    # knows what happened.
    logger.info("Fetching models for step 2 display")
    available_models = get_formatted_model_list(api_key)
    if not available_models:
        logger.error("Failed to fetch models for step 2 display")
        return render(request, 'setup/step2.html', {
            'error': 'Could not fetch models from OpenRouter. Go back and check your API key.',
            'available_models': [],
            'available_models_json': '[]',
            'themes': get_theme_list(),
            'themes_json': json.dumps(get_theme_list()),
            'selected_theme': config.get('THEME', 'liminal-salt'),
            'selected_mode': config.get('THEME_MODE', 'dark'),
        })

    themes = get_theme_list()
    return render(request, 'setup/step2.html', {
        'model_count': len(available_models),
        'available_models': available_models,
        'available_models_json': json.dumps(available_models),
        'selected_model': config.get('MODEL', ''),
        'themes': themes,
        'themes_json': json.dumps(themes),
        'selected_theme': config.get('THEME', 'liminal-salt'),
        'selected_mode': config.get('THEME_MODE', 'dark'),
    })


def _setup_step3(request, config):
    """Step 3 — agreement. Accepting writes both flags and finishes setup."""
    config = config or {}

    if request.method == 'POST' and request.POST.get('setup_action') == 'accept':
        config['SETUP_COMPLETE'] = True
        config['AGREEMENT_ACCEPTED'] = CURRENT_AGREEMENT_VERSION
        save_config(config)
        logger.info(f"Setup complete; agreement version {CURRENT_AGREEMENT_VERSION} accepted")

        del request.session['setup_step']
        request.session.modified = True
        return redirect('chat')

    # Back button shown only during the initial walk-through. If setup was
    # previously complete and we're re-prompting for a new agreement, the
    # user has no earlier steps to go back to.
    can_go_back = not config.get('SETUP_COMPLETE')

    return render(request, 'setup/step3.html', {
        'agreement_version': CURRENT_AGREEMENT_VERSION,
        'agreement_body': AGREEMENT_BODY,
        'can_go_back': can_go_back,
    })
