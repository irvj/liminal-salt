import requests
import logging

logger = logging.getLogger(__name__)

# Bump this whenever the user agreement copy materially changes. Users whose
# stored AGREEMENT_ACCEPTED doesn't match the current version will be kicked
# back to the agreement step (setup step 3) without having to redo provider
# or model selection.
CURRENT_AGREEMENT_VERSION = "1.0"


def is_app_ready(config):
    """
    App is accessible only when setup has finished AND the user has accepted
    the current agreement version. Either missing → wizard.
    """
    if not config:
        return False
    return (
        config.get("SETUP_COMPLETE") is True
        and config.get("AGREEMENT_ACCEPTED") == CURRENT_AGREEMENT_VERSION
    )


# Available API providers
# Each provider has: id, name, api_key_url, api_key_placeholder
PROVIDERS = [
    {
        "id": "openrouter",
        "name": "OpenRouter",
        "api_key_url": "https://openrouter.ai/keys",
        "api_key_placeholder": "sk-or-v1-..."
    },
]


def get_providers():
    """Return list of available providers."""
    return PROVIDERS


def get_provider_by_id(provider_id):
    """Get a provider by its ID."""
    for provider in PROVIDERS:
        if provider["id"] == provider_id:
            return provider
    return None

def validate_api_key(api_key):
    """
    Validate an OpenRouter API key by checking the auth endpoint.

    Args:
        api_key: OpenRouter API key to validate

    Returns:
        True if valid, False otherwise
    """
    try:
        logger.info("Validating API key with OpenRouter...")
        response = requests.get(
            "https://openrouter.ai/api/v1/auth/key",
            headers={"Authorization": f"Bearer {api_key}"},
            timeout=10
        )
        logger.info(f"OpenRouter auth response status: {response.status_code}")

        if response.status_code == 200:
            data = response.json().get("data", {})
            logger.info(f"API key valid. Label: {data.get('label', 'N/A')}")
            return True
        else:
            logger.error(f"API key validation failed: {response.status_code}")
            return False
    except requests.exceptions.Timeout:
        logger.error("Timeout while validating API key")
        return False
    except requests.exceptions.RequestException as e:
        logger.error(f"Network error while validating API key: {str(e)}")
        return False
    except Exception as e:
        logger.error(f"Unexpected error validating API key: {str(e)}")
        return False


def fetch_available_models(api_key):
    """
    Fetch list of available models from OpenRouter API

    Args:
        api_key: OpenRouter API key

    Returns:
        List of model dicts with 'id' and 'name', or None if error
    """
    try:
        logger.info("Fetching models from OpenRouter API...")
        response = requests.get(
            "https://openrouter.ai/api/v1/models",
            headers={"Authorization": f"Bearer {api_key}"},
            timeout=10
        )
        logger.info(f"OpenRouter API response status: {response.status_code}")

        if response.status_code == 200:
            models = response.json().get("data", [])
            logger.info(f"Successfully fetched {len(models)} models")
            # Return list with id, name, pricing, and context_length
            return [{
                "id": m["id"],
                "name": m.get("name", m["id"]),
                "pricing": m.get("pricing", {}),
                "context_length": m.get("context_length", 0)
            } for m in models]
        else:
            logger.error(f"OpenRouter API returned status {response.status_code}: {response.text[:200]}")
            return None
    except requests.exceptions.Timeout:
        logger.error("Timeout while connecting to OpenRouter API")
        return None
    except requests.exceptions.RequestException as e:
        logger.error(f"Network error while fetching models: {str(e)}")
        return None
    except Exception as e:
        logger.error(f"Unexpected error in fetch_available_models: {str(e)}")
        return None
