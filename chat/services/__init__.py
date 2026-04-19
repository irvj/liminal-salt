# Business logic services
from .chat_core import ChatCore
from .session_manager import (
    load_session, create_session, delete_session,
    get_session_persona, get_session_draft,
    toggle_pin, rename_session, save_draft, clear_draft,
    remove_last_assistant_message, update_last_user_message,
    update_persona_across_sessions, get_session_path,
    generate_session_id, make_user_timestamp,
)
from .persona_manager import (
    get_persona_preview, save_persona_identity,
    create_persona as create_persona_dir,
    delete_persona as delete_persona_dir,
    rename_persona, persona_exists,
)
from .config_manager import (
    fetch_available_models, validate_api_key, get_providers,
    is_app_ready, CURRENT_AGREEMENT_VERSION,
)
from .context_manager import load_context, get_available_personas, get_persona_config, save_persona_config, get_persona_model, get_persona_identity, ensure_default_personas
from .llm_client import call_llm, LLMError
from .memory_manager import (
    MemoryManager,
    get_memory_file, get_memory_content, save_memory_content,
    delete_memory, rename_memory, list_persona_memories,
    get_memory_model,
)
from .summarizer import Summarizer
from .context_files import ContextFileManager
from .local_context import browse_directory

from django.conf import settings as _django_settings

# ---------------------------------------------------------------------------
# Global (user-level) context files — singleton instance
# ---------------------------------------------------------------------------
_global_context = ContextFileManager(
    base_dir=_django_settings.DATA_DIR / 'user_context',
    scope_label="USER",
    header_description="The following files were provided by the user as additional context.",
)

def get_user_context_dir():
    return _global_context._ensure_dir()

list_context_files          = _global_context.list_files
upload_context_file         = _global_context.upload_file
delete_context_file         = _global_context.delete_file
toggle_context_file         = _global_context.toggle_file
load_enabled_context        = _global_context.load_enabled_context
get_context_file_content    = _global_context.get_file_content
save_context_file_content   = _global_context.save_file_content
add_context_local_directory      = _global_context.add_local_directory
remove_context_local_directory   = _global_context.remove_local_directory
list_context_local_directories   = _global_context.list_local_directories
toggle_context_local_file        = _global_context.toggle_local_file
get_context_local_file_content   = _global_context.get_local_file_content
refresh_context_local_directory  = _global_context.refresh_local_directory

# ---------------------------------------------------------------------------
# Per-persona context files — factory function creates scoped instances
# ---------------------------------------------------------------------------
import os as _os

def _persona_ctx(persona_name):
    """Get a ContextFileManager scoped to a specific persona."""
    persona_name = _os.path.basename(persona_name)
    return ContextFileManager(
        base_dir=_django_settings.DATA_DIR / 'user_context' / 'personas' / persona_name,
        scope_label="PERSONA",
        header_description="The following files provide additional context for this persona.",
    )

def list_persona_context_files(persona_name):
    return _persona_ctx(persona_name).list_files()

def upload_persona_context_file(persona_name, uploaded_file):
    return _persona_ctx(persona_name).upload_file(uploaded_file)

def delete_persona_context_file(persona_name, filename):
    return _persona_ctx(persona_name).delete_file(filename)

def toggle_persona_context_file(persona_name, filename, enabled=None):
    return _persona_ctx(persona_name).toggle_file(filename, enabled)

def get_persona_context_file_content(persona_name, filename):
    return _persona_ctx(persona_name).get_file_content(filename)

def save_persona_context_file_content(persona_name, filename, content):
    return _persona_ctx(persona_name).save_file_content(filename, content)

def load_enabled_persona_context(persona_name):
    return _persona_ctx(persona_name).load_enabled_context()

def add_persona_context_local_directory(persona_name, dir_path):
    return _persona_ctx(persona_name).add_local_directory(dir_path)

def remove_persona_context_local_directory(persona_name, dir_path):
    return _persona_ctx(persona_name).remove_local_directory(dir_path)

def list_persona_context_local_directories(persona_name):
    return _persona_ctx(persona_name).list_local_directories()

def toggle_persona_context_local_file(persona_name, dir_path, filename, enabled=None):
    return _persona_ctx(persona_name).toggle_local_file(dir_path, filename, enabled)

def get_persona_context_local_file_content(persona_name, dir_path, filename):
    return _persona_ctx(persona_name).get_local_file_content(dir_path, filename)

def refresh_persona_context_local_directory(persona_name, dir_path):
    return _persona_ctx(persona_name).refresh_local_directory(dir_path)
