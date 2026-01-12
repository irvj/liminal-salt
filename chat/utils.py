"""
Utility functions for chat app
"""
import os
import json
import logging
from pathlib import Path
from collections import defaultdict, OrderedDict
from django.conf import settings

logger = logging.getLogger(__name__)


def load_config():
    """Load configuration from config.json"""
    config_path = settings.CONFIG_FILE
    if not os.path.exists(config_path):
        return {}
    try:
        with open(config_path, 'r') as f:
            return json.load(f)
    except json.JSONDecodeError as e:
        logger.error(f"Config file corrupted: {e}")
        return {}
    except Exception as e:
        logger.error(f"Error loading config: {e}")
        return {}


def save_config(config_data):
    """Save configuration to config.json with explicit flush"""
    config_path = settings.CONFIG_FILE
    with open(config_path, 'w') as f:
        json.dump(config_data, f, indent=4)
        f.flush()
        os.fsync(f.fileno())


def get_sessions_with_titles():
    """
    Get list of all sessions with their titles, personas, and pinned status
    Returns list of dicts: [{"id": "session_*.json", "title": "...", "persona": "...", "pinned": bool}]
    """
    sessions_dir = settings.SESSIONS_DIR
    os.makedirs(sessions_dir, exist_ok=True)

    sessions = []
    files = [f for f in os.listdir(sessions_dir) if f.endswith(".json")]

    for f in files:
        path = os.path.join(sessions_dir, f)
        try:
            with open(path, 'r') as file:
                data = json.load(file)
                title = data.get("title", "New Chat") if isinstance(data, dict) else "Old Session"
                persona = data.get("persona", "assistant") if isinstance(data, dict) else "assistant"
                pinned = data.get("pinned", False) if isinstance(data, dict) else False
                sessions.append({"id": f, "title": title, "persona": persona, "pinned": pinned})
        except Exception:
            sessions.append({"id": f, "title": "Error Loading", "persona": "assistant", "pinned": False})

    return sorted(sessions, key=lambda x: x['id'], reverse=True)


def group_sessions_by_persona(sessions):
    """
    Group sessions by persona, maintaining chronological order within groups.
    Order personas by most recent thread. Also returns pinned sessions separately.

    Args:
        sessions: List of session dicts from get_sessions_with_titles()

    Returns:
        Tuple of (pinned_sessions, ordered_groups)
        - pinned_sessions: List of pinned sessions (sorted newest-first)
        - ordered_groups: OrderedDict mapping persona -> list of non-pinned sessions
    """
    # Separate pinned and unpinned sessions
    pinned_sessions = [s for s in sessions if s.get("pinned", False)]
    unpinned_sessions = [s for s in sessions if not s.get("pinned", False)]

    # Group unpinned sessions by persona
    groups = defaultdict(list)
    for session in unpinned_sessions:
        groups[session["persona"]].append(session)

    # Sort personas by most recent thread (sessions already sorted newest-first)
    persona_order = sorted(
        groups.keys(),
        key=lambda p: groups[p][0]["id"] if groups[p] else "",
        reverse=True
    )

    # Create ordered dict
    ordered_groups = OrderedDict()
    for persona in persona_order:
        ordered_groups[persona] = groups[persona]

    return pinned_sessions, ordered_groups


def get_current_session(request):
    """Get current session ID from Django session"""
    return request.session.get('current_session')


def set_current_session(request, session_id):
    """Set current session ID in Django session"""
    request.session['current_session'] = session_id
    request.session.modified = True


def get_collapsed_personas(request):
    """Get collapsed personas dict from Django session"""
    return request.session.get('collapsed_personas', {})


def set_collapsed_personas(request, collapsed_dict):
    """Set collapsed personas dict in Django session"""
    request.session['collapsed_personas'] = collapsed_dict
    request.session.modified = True


def toggle_persona_group(request, persona):
    """Toggle collapse state for a persona group"""
    collapsed = get_collapsed_personas(request)
    current = collapsed.get(persona, False)
    collapsed[persona] = not current
    set_collapsed_personas(request, collapsed)


def aggregate_all_sessions_messages():
    """
    Collect all messages from all session files for comprehensive memory update
    Returns list of all messages across all sessions
    """
    sessions_dir = settings.SESSIONS_DIR
    all_messages = []

    for session_file in os.listdir(sessions_dir):
        if session_file.endswith(".json"):
            try:
                path = os.path.join(sessions_dir, session_file)
                with open(path, 'r') as f:
                    data = json.load(f)
                    messages = data.get("messages", []) if isinstance(data, dict) else data
                    if isinstance(messages, list):
                        all_messages.extend(messages)
            except Exception as e:
                print(f"Error reading session {session_file}: {e}")
                continue

    return all_messages


def title_has_artifacts(title):
    """Check if title needs regeneration due to artifacts"""
    if not title or title == "New Chat" or title == "":
        return False
    bad_patterns = ['[', ']', '<', '>', '#', '\n', 'Prompt', 'INST', 'SYS', '###']
    return any(pattern in title for pattern in bad_patterns)


def ensure_sessions_dir():
    """Ensure sessions directory exists"""
    os.makedirs(settings.SESSIONS_DIR, exist_ok=True)


def format_model_pricing(pricing):
    """
    Format pricing information for display

    Args:
        pricing: Dict with 'prompt' and 'completion' keys (strings)

    Returns:
        Formatted string like "$3.00/$15.00 per 1M" or "Free"
    """
    if not pricing:
        return ""

    prompt_cost = float(pricing.get("prompt", 0))
    completion_cost = float(pricing.get("completion", 0))

    # Check if free
    if prompt_cost == 0 and completion_cost == 0:
        return "Free"

    # Convert from per-token to per-million-tokens for readability
    prompt_per_million = prompt_cost * 1_000_000
    completion_per_million = completion_cost * 1_000_000

    # Format with appropriate precision
    if prompt_per_million < 0.01:
        prompt_str = f"${prompt_per_million:.4f}"
    else:
        prompt_str = f"${prompt_per_million:.2f}"

    if completion_per_million < 0.01:
        completion_str = f"${completion_per_million:.4f}"
    else:
        completion_str = f"${completion_per_million:.2f}"

    return f"{prompt_str}/{completion_str} per 1M"


def group_models_by_provider(models):
    """
    Group models by provider (extracted from model ID)

    Args:
        models: List of model dicts with 'id' and 'name'

    Returns:
        Dict mapping provider -> list of models
    """
    groups = defaultdict(list)
    for model in models:
        provider = model["id"].split("/")[0] if "/" in model["id"] else "Other"
        groups[provider].append(model)

    # Sort providers alphabetically, sort models within each group
    sorted_groups = {}
    for provider in sorted(groups.keys()):
        sorted_groups[provider] = sorted(groups[provider], key=lambda m: m["name"])

    return sorted_groups


def flatten_models_with_provider_prefix(grouped_models):
    """
    Flatten grouped models with provider prefix and pricing

    Args:
        grouped_models: Dict from group_models_by_provider()

    Returns:
        List of tuples: [(model_id, display_name), ...]
    """
    options = []
    for provider, models in grouped_models.items():
        # Capitalize provider name for display
        provider_display = provider.replace('-', ' ').title()
        for model in models:
            model_name = model['name']
            # Skip adding provider prefix if model name already starts with it
            if not model_name.lower().startswith(provider.lower()):
                model_name = f"{provider_display}: {model_name}"

            pricing_str = format_model_pricing(model.get("pricing", {}))
            if pricing_str:
                display_name = f"{model_name} - {pricing_str}"
            else:
                display_name = model_name
            options.append((model["id"], display_name))
    return options
