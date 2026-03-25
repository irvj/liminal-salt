import requests
import json

OPENROUTER_API_URL = "https://openrouter.ai/api/v1/chat/completions"


class LLMError(Exception):
    """Raised when an LLM API call fails."""
    pass


def call_llm(api_key, model, messages, site_url=None, site_name=None, timeout=30):
    """
    Make a single LLM API call to OpenRouter.

    Args:
        api_key: OpenRouter API key
        model: Model identifier string
        messages: List of message dicts (role/content)
        site_url: Optional HTTP-Referer header
        site_name: Optional X-Title header
        timeout: Request timeout in seconds

    Returns:
        Response content string (cleaned of token artifacts)

    Raises:
        LLMError: On API failure, empty response, or timeout
    """
    headers = {
        "Authorization": f"Bearer {api_key}",
        "Content-Type": "application/json"
    }
    if site_url:
        headers["HTTP-Referer"] = site_url
    if site_name:
        headers["X-Title"] = site_name

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
