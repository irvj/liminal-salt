import json
import os
import requests

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
        response = requests.get(
            "https://openrouter.ai/api/v1/models",
            headers={"Authorization": f"Bearer {api_key}"},
            timeout=10
        )
        if response.status_code == 200:
            models = response.json().get("data", [])
            # Return simplified list with id and name
            return [{"id": m["id"], "name": m.get("name", m["id"])} for m in models]
        else:
            return None
    except Exception:
        return None
