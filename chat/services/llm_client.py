import requests
import json

OPENROUTER_API_URL = "https://openrouter.ai/api/v1/chat/completions"

# OpenRouter app attribution — hardcoded here because this module is the sole
# path to OpenRouter. See https://openrouter.ai/docs/app-attribution.
OPENROUTER_APP_URL = "https://liminalsalt.app"
OPENROUTER_APP_NAME = "Liminal Salt"
OPENROUTER_APP_CATEGORIES = "general-chat,roleplay"


class LLMError(Exception):
    """Raised when an LLM API call fails."""
    pass


def call_llm(api_key, model, messages, timeout=30):
    """
    Make a single LLM API call to OpenRouter.

    Args:
        api_key: OpenRouter API key
        model: Model identifier string
        messages: List of message dicts (role/content)
        timeout: Request timeout in seconds

    Returns:
        Response content string (cleaned of token artifacts)

    Raises:
        LLMError: On API failure, empty response, or timeout
    """
    headers = {
        "Authorization": f"Bearer {api_key}",
        "Content-Type": "application/json",
        "HTTP-Referer": OPENROUTER_APP_URL,
        "X-OpenRouter-Title": OPENROUTER_APP_NAME,
        "X-OpenRouter-Categories": OPENROUTER_APP_CATEGORIES,
    }

    payload = {
        "model": model,
        "messages": messages
    }

    try:
        response = requests.post(
            url=OPENROUTER_API_URL,
            headers=headers,
            data=json.dumps(payload),
            timeout=timeout
        )
        response.raise_for_status()
    except requests.exceptions.Timeout:
        raise LLMError("Request timed out")
    except requests.exceptions.RequestException as e:
        raise LLMError(f"API request failed: {e}")

    try:
        data = response.json()
    except ValueError:
        raise LLMError("Invalid JSON in API response")

    if 'choices' not in data or not data['choices']:
        raise LLMError("No choices in API response")

    content = data['choices'][0].get('message', {}).get('content')
    if not content:
        raise LLMError("Empty content in API response")

    # Clean token artifacts
    content = content.replace('<s>', '').replace('</s>', '').strip()
    return content
