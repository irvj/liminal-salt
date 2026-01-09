import json
import os
import requests
import logging

logger = logging.getLogger(__name__)

def load_config(path="config.json"):
    if not os.path.exists(path):
        return {}
    with open(path, 'r') as f:
        return json.load(f)

def save_config(config_data, path="config.json"):
    """Save configuration to JSON file"""
    with open(path, 'w') as f:
        json.dump(config_data, f, indent=4)

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
