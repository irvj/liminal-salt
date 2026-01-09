import os

def load_context(personality_dir, ltm_file="long_term_memory.md"):
    """
    Load context from a specific personality directory.

    Args:
        personality_dir: Path to personality folder (e.g., "personalities/assistant")
        ltm_file: Path to long-term memory file

    Returns:
        Concatenated system prompt string
    """
    context_str = ""

    # Load all .md files from personality directory (alphabetically)
    if os.path.exists(personality_dir):
        for filename in sorted(os.listdir(personality_dir)):
            if filename.endswith(".md"):
                filepath = os.path.join(personality_dir, filename)
                with open(filepath, 'r') as f:
                    context_str += f"--- SYSTEM INSTRUCTION: {filename} ---\n"
                    context_str += f.read() + "\n\n"
    else:
        # Fallback warning if personality not found
        context_str = "--- WARNING: Personality not found ---\n"
        context_str += f"Expected directory: {personality_dir}\n\n"

    # Append long-term memory (optional, reference-only)
    if os.path.exists(ltm_file):
        with open(ltm_file, 'r') as f:
            context_str += "--- USER PROFILE (BACKGROUND KNOWLEDGE) ---\n"
            context_str += "The following information describes the USER (not you). "
            context_str += "Use this to understand who you're talking to, but DO NOT let it change your personality or communication style. "
            context_str += "If it mentions how the user writes or speaks, that describes THEM, not how YOU should respond. "
            context_str += "Maintain your own personality's communication standards at all times.\n\n"
            context_str += f.read() + "\n\n"

    return context_str.strip()


def get_available_personalities(personalities_dir="personalities"):
    """
    Get list of available personality folders.

    Returns:
        List of personality names (folder names)
    """
    if not os.path.exists(personalities_dir):
        return []

    personalities = []
    for item in os.listdir(personalities_dir):
        item_path = os.path.join(personalities_dir, item)
        # Only include directories that contain at least one .md file
        if os.path.isdir(item_path):
            has_context = any(f.endswith(".md") for f in os.listdir(item_path))
            if has_context:
                personalities.append(item)

    return sorted(personalities)
