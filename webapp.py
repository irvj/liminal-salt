import streamlit as st
import os
import json
from datetime import datetime
from config_manager import load_config
from context_manager import load_context, get_available_personalities
from chat_core import ChatCore
from summarizer import Summarizer

# Page Config
st.set_page_config(page_title="MVP Multi-Session Chatbot", page_icon="🤖", layout="wide")

# Load Configuration
config = load_config()
api_key = config.get("OPENROUTER_API_KEY")
model = config.get("MODEL", "google/gemini-2.0-flash-exp:free")
max_history = config.get("MAX_HISTORY", 50)
ltm_file = config.get("LTM_FILE", "long_term_memory.md")
sessions_dir = config.get("SESSIONS_DIR", "sessions")

# Ensure sessions directory exists
os.makedirs(sessions_dir, exist_ok=True)

# Helper: Get list of sessions with titles and personalities
def get_sessions_with_titles():
    sessions = []
    files = [f for f in os.listdir(sessions_dir) if f.endswith(".json")]
    for f in files:
        path = os.path.join(sessions_dir, f)
        try:
            with open(path, 'r') as file:
                data = json.load(file)
                title = data.get("title", "New Chat") if isinstance(data, dict) else "Old Session"
                personality = data.get("personality", "assistant") if isinstance(data, dict) else "assistant"
                sessions.append({"id": f, "title": title, "personality": personality})
        except Exception:
            sessions.append({"id": f, "title": "Error Loading", "personality": "assistant"})
    return sorted(sessions, key=lambda x: x['id'], reverse=True)

def group_sessions_by_personality(sessions):
    """
    Group sessions by personality, maintaining chronological order within groups.
    Order personalities by most recent thread.
    """
    from collections import OrderedDict, defaultdict

    # Group sessions by personality
    groups = defaultdict(list)
    for session in sessions:
        groups[session["personality"]].append(session)

    # Sort personalities by most recent thread (sessions already sorted newest-first)
    personality_order = sorted(
        groups.keys(),
        key=lambda p: groups[p][0]["id"] if groups[p] else "",
        reverse=True
    )

    # Create ordered dict
    ordered_groups = OrderedDict()
    for personality in personality_order:
        ordered_groups[personality] = groups[personality]

    return ordered_groups

def toggle_personality_group(personality):
    """Toggle collapse state for a personality group."""
    current = st.session_state.collapsed_personalities.get(personality, False)
    st.session_state.collapsed_personalities[personality] = not current

def _title_has_artifacts(title):
    """Check if title needs regeneration"""
    if not title or title == "New Chat" or title == "":
        return False
    bad_patterns = ['[', ']', '<', '>', '#', '\n', 'Prompt', 'INST', 'SYS', '###']
    return any(pattern in title for pattern in bad_patterns)

def aggregate_all_sessions_messages():
    """Collect all messages from all session files for comprehensive memory update"""
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

# Delete confirmation dialog
@st.dialog("Delete Chat Thread?")
def confirm_delete_dialog(session_id, session_title):
    st.write(f"Are you sure you want to delete **{session_title}**?")
    st.write("This action cannot be undone.")

    col1, col2 = st.columns(2)
    with col1:
        if st.button("Cancel", use_container_width=True, key="cancel_delete"):
            st.rerun()
    with col2:
        if st.button("Delete", type="primary", use_container_width=True, key="confirm_delete_btn"):
            # Perform deletion
            path = os.path.join(sessions_dir, session_id)
            if os.path.exists(path):
                os.remove(path)

            # Handle switching to another session if current was deleted
            is_current = session_id == st.session_state.current_session
            if is_current:
                remaining = [sess for sess in get_sessions_with_titles() if sess["id"] != session_id]
                if remaining:
                    st.session_state.current_session = remaining[0]["id"]
                else:
                    new_id = f"session_{datetime.now().strftime('%Y%m%d_%H%M%S')}.json"
                    st.session_state.current_session = new_id
                load_chat_core()

            st.rerun()

# Wipe memory confirmation dialog
@st.dialog("Wipe Memory?")
def confirm_wipe_memory_dialog():
    st.write("Are you sure you want to wipe your **entire long-term memory**?")
    st.write("This action cannot be undone.")

    col1, col2 = st.columns(2)
    with col1:
        if st.button("Cancel", use_container_width=True, key="cancel_wipe_memory"):
            st.rerun()
    with col2:
        if st.button("Wipe Memory", type="primary", use_container_width=True, key="confirm_wipe_memory"):
            # Delete memory file
            if os.path.exists(ltm_file):
                os.remove(ltm_file)
            # Reset timestamp
            st.session_state.last_memory_update = None
            st.toast("Memory wiped successfully", icon="🗑️")
            st.rerun()

# New chat creation dialog
@st.dialog("New Chat")
def create_new_chat_dialog():
    st.write("Choose a personality for this conversation:")

    personalities_dir = config.get("PERSONALITIES_DIR", "personalities")
    available_personalities = get_available_personalities(personalities_dir)
    default_personality = config.get("DEFAULT_PERSONALITY", "assistant")

    selected_personality = st.selectbox(
        "Personality",
        options=available_personalities,
        index=available_personalities.index(default_personality)
              if default_personality in available_personalities
              else 0,
        key="new_chat_personality_selector"
    )

    col1, col2 = st.columns(2)
    with col1:
        if st.button("Cancel", use_container_width=True, key="cancel_new_chat"):
            st.rerun()
    with col2:
        if st.button("Create", type="primary", use_container_width=True, key="confirm_new_chat"):
            # Create new session with selected personality
            new_id = f"session_{datetime.now().strftime('%Y%m%d_%H%M%S')}.json"
            st.session_state.current_session = new_id
            st.session_state.view_mode = "chat"

            # Store selected personality for this session
            st.session_state.session_personalities[new_id] = selected_personality

            load_chat_core()
            st.rerun()


# Initialize Session State
if "current_session" not in st.session_state:
    sessions = get_sessions_with_titles()
    if sessions:
        st.session_state.current_session = sessions[0]["id"]
    else:
        new_id = f"session_{datetime.now().strftime('%Y%m%d_%H%M%S')}.json"
        st.session_state.current_session = new_id

# Initialize memory update state from file modification time
if "last_memory_update" not in st.session_state:
    if os.path.exists(ltm_file):
        # Get file modification time
        mtime = os.path.getmtime(ltm_file)
        st.session_state.last_memory_update = datetime.fromtimestamp(mtime)
    else:
        st.session_state.last_memory_update = None

# Clean up orphaned status file from old threading implementation
if os.path.exists('.memory_update_status'):
    try:
        os.remove('.memory_update_status')
        print("[Startup] Cleaned up orphaned .memory_update_status file")
    except:
        pass

# Initialize view mode state (chat or profile or settings)
if "view_mode" not in st.session_state:
    st.session_state.view_mode = "chat"

# Initialize session personalities mapping (session_id -> personality)
if "session_personalities" not in st.session_state:
    st.session_state.session_personalities = {}

# Initialize collapsed state for personality groups
if "collapsed_personalities" not in st.session_state:
    st.session_state.collapsed_personalities = {}

# Function to load/reload chat core
def load_chat_core():
    personalities_dir = config.get("PERSONALITIES_DIR", "personalities")
    history_path = os.path.join(sessions_dir, st.session_state.current_session)

    # Try to load personality from session file
    session_personality = None
    if os.path.exists(history_path):
        try:
            with open(history_path, 'r') as f:
                data = json.load(f)
                if isinstance(data, dict):
                    session_personality = data.get("personality")
        except:
            pass

    # Fallback: check in-memory mapping, then default setting
    if not session_personality:
        session_personality = st.session_state.session_personalities.get(
            st.session_state.current_session,
            config.get("DEFAULT_PERSONALITY", "assistant")
        )

    personality_path = os.path.join(personalities_dir, session_personality)
    system_prompt = load_context(personality_path, ltm_file=ltm_file)

    st.session_state.chat = ChatCore(
        api_key=api_key,
        model=model,
        system_prompt=system_prompt,
        max_history=max_history,
        history_file=history_path,
        personality=session_personality
    )
    st.session_state.last_loaded_session = st.session_state.current_session
    st.session_state.summarizer = Summarizer(api_key, model)

# Load chat core if session changed or not initialized
if "chat" not in st.session_state or st.session_state.get("last_loaded_session") != st.session_state.current_session:
    load_chat_core()

# Sidebar: Session Management & User Profile
with st.sidebar:
    # Title and New Chat button on same row
    col1, col2 = st.columns([0.85, 0.15])
    with col1:
        st.title("Chat Threads")
    with col2:
        # Align button to top with empty space
        st.write("")  # Add spacing to align with title
        if st.button("➕", key="new_chat_btn", type="primary", help="New Chat"):
            create_new_chat_dialog()

    st.divider()

    # Session List with Personality Groups
    sessions = get_sessions_with_titles()
    grouped_sessions = group_sessions_by_personality(sessions)

    for personality, personality_sessions in grouped_sessions.items():
        # Personality Header (Collapsible)
        is_collapsed = st.session_state.collapsed_personalities.get(personality, False)
        arrow = "▶" if is_collapsed else "▼"
        count = len(personality_sessions)
        display_name = personality.capitalize()

        # Header button
        if st.button(
            f"{arrow} {display_name} ({count})",
            key=f"toggle_{personality}",
            use_container_width=True,
            type="secondary"
        ):
            toggle_personality_group(personality)
            st.rerun()

        # Show threads if expanded
        if not is_collapsed:
            for s in personality_sessions:
                # Three-column layout: [indent | title | delete]
                col_indent, col_title, col_delete = st.columns([0.02, 0.83, 0.15])

                is_current = (
                    s["id"] == st.session_state.current_session and
                    st.session_state.view_mode == "chat"
                )

                with col_indent:
                    st.empty()

                with col_title:
                    display_title = f"**{s['title']}**" if is_current else s['title']
                    if st.button(display_title, key=f"select_{s['id']}", use_container_width=True):
                        st.session_state.current_session = s["id"]
                        st.session_state.view_mode = "chat"
                        load_chat_core()
                        st.rerun()

                with col_delete:
                    if st.button("🗑️", key=f"del_{s['id']}", help="Delete session"):
                        confirm_delete_dialog(s["id"], s["title"])

    st.divider()

    # View Profile Button
    if st.button("User Memory", use_container_width=True, type="primary" if st.session_state.view_mode == "profile" else "secondary"):
        st.session_state.view_mode = "profile"
        st.rerun()

    # Settings Button
    if st.button("Settings", use_container_width=True, type="primary" if st.session_state.view_mode == "settings" else "secondary"):
        st.session_state.view_mode = "settings"
        st.rerun()

# Main View - Chat or Profile or Settings
if st.session_state.view_mode == "profile":
    # Profile View
    st.title("User Memory")
    st.caption(f"Model: {model}")

    # Memory management buttons in columns
    col1, col2 = st.columns(2)

    with col1:
        # Update button - simple blocking approach
        if st.button("Update User Memory", key="update_memory_btn", use_container_width=True):
            with st.spinner("Updating memory from all sessions..."):
                try:
                    # Aggregate messages from all sessions
                    all_messages = aggregate_all_sessions_messages()

                    if not all_messages:
                        st.warning("No messages found in any session.", icon="⚠️")
                    else:
                        # Update memory synchronously (blocks UI)
                        st.session_state.summarizer.update_long_term_memory(all_messages, ltm_file)

                        # Update timestamp
                        st.session_state.last_memory_update = datetime.now()

                        # Show success toast
                        st.toast("Memory updated successfully!", icon="✅")

                except Exception as e:
                    st.toast(f"Memory update failed: {str(e)}", icon="❌")

            # Rerun to refresh displayed content
            st.rerun()

    with col2:
        # Wipe button with popup confirmation
        if st.button("Wipe Memory", key="wipe_memory_btn", type="secondary", use_container_width=True):
            confirm_wipe_memory_dialog()

    # Show update status
    if st.session_state.last_memory_update:
        # Format: "Jan 8, 2026 at 3:45 PM"
        datetime_str = st.session_state.last_memory_update.strftime("%b %d, %Y at %I:%M %p")
        st.caption(f"Last updated: {datetime_str}")
    else:
        st.caption("Never updated from all sessions")

    st.divider()

    if os.path.exists(ltm_file):
        with open(ltm_file, 'r') as f:
            content = f.read()
        st.markdown(content)
    else:
        st.info("No long-term memory found yet. Memory will be created after your first update.")

elif st.session_state.view_mode == "settings":
    # Settings View
    st.title("Settings")
    st.caption(f"Model: {model}")

    st.divider()

    # Personality Selection
    st.subheader("Chatbot Personality")

    personalities_dir = config.get("PERSONALITIES_DIR", "personalities")
    available_personalities = get_available_personalities(personalities_dir)

    if not available_personalities:
        st.error(f"No personalities found in '{personalities_dir}/' directory.")
        st.info("Create personality folders with .md files to define chatbot personalities.")
    else:
        # Default personality info
        default_personality = config.get("DEFAULT_PERSONALITY", "assistant")
        st.write(f"**Default personality for new chats:** {default_personality}")

        # Personality selector
        selected_personality = st.selectbox(
            "Select a personality",
            options=available_personalities,
            index=available_personalities.index(default_personality)
                  if default_personality in available_personalities
                  else 0,
            key="personality_selector"
        )

        # Set as default button
        if st.button("Set as Default", type="primary", use_container_width=True):
            if selected_personality != default_personality:
                # Update config file
                config["DEFAULT_PERSONALITY"] = selected_personality
                with open("config.json", 'w') as f:
                    json.dump(config, f, indent=4)

                st.toast(f"Default personality set to '{selected_personality}'", icon="✅")
                st.rerun()
            else:
                st.info("This personality is already the default.")

        st.divider()

        # Show personality description (first 500 chars of first .md file)
        personality_path = os.path.join(personalities_dir, selected_personality)
        if os.path.exists(personality_path):
            md_files = [f for f in os.listdir(personality_path) if f.endswith(".md")]
            if md_files:
                with open(os.path.join(personality_path, md_files[0]), 'r') as f:
                    content = f.read()
                    preview = content[:500] + ("..." if len(content) > 500 else "")
                    with st.expander("Preview Personality Context"):
                        st.text(preview)

else:
    # Main Chat Interface
    st.title("🤖 MVP Chatbot")
    st.caption(f"Active Session: {st.session_state.chat.title} | Personality: {st.session_state.chat.personality} | Model: {model}")

    # Display Chat History
    for message in st.session_state.chat.messages:
        with st.chat_message(message["role"]):
            if message["content"].startswith("ERROR:"):
                error_msg = message["content"].replace("ERROR:", "").strip()
                st.error("Something went wrong", icon="⚠️")
                with st.expander("View Error Details"):
                    st.code(error_msg)
            else:
                st.markdown(message["content"])

    # Chat Input
    if prompt := st.chat_input("What's on your mind?"):
        # Display user message immediately
        with st.chat_message("user"):
            st.markdown(prompt)

        # Generate response
        with st.chat_message("assistant"):
            with st.spinner("Thinking..."):
                response = st.session_state.chat.send_message(prompt)

                if response.startswith("ERROR:"):
                    error_msg = response.replace("ERROR:", "").strip()
                    st.error("Something went wrong", icon="⚠️")
                    with st.expander("View Error Details"):
                        st.code(error_msg)
                else:
                    st.markdown(response)

        # Helper to get first user message
        def _get_first_user_message():
            for msg in st.session_state.chat.messages:
                if msg["role"] == "user":
                    return msg["content"]
            return ""

        # TIER 1: First message - attempt title generation ONLY if we have a valid response
        # If retries failed, leave as "New Chat" and let Tier 2 handle it on next valid response
        if (st.session_state.chat.title == "New Chat" and
            len(st.session_state.chat.messages) <= 2 and
            not response.startswith("ERROR:")):
            with st.spinner("Naming session..."):
                first_user_msg = _get_first_user_message()
                new_title = st.session_state.summarizer.generate_title(first_user_msg, response)
                st.session_state.chat.title = new_title
                st.session_state.chat._save_history()
            st.rerun()

        # TIER 2: Retry logic - if title is still "New Chat" after 2nd successful response
        elif (st.session_state.chat.title == "New Chat" and
              len(st.session_state.chat.messages) > 2 and
              not response.startswith("ERROR:")):
            with st.spinner("Naming session..."):
                first_user_msg = _get_first_user_message()
                new_title = st.session_state.summarizer.generate_title(first_user_msg, response)
                st.session_state.chat.title = new_title
                st.session_state.chat._save_history()
            st.rerun()

        # TIER 3: Fix malformed titles - if title contains artifacts and we have valid response
        elif (_title_has_artifacts(st.session_state.chat.title) and
              not response.startswith("ERROR:")):
            with st.spinner("Fixing session name..."):
                first_user_msg = _get_first_user_message()
                new_title = st.session_state.summarizer.generate_title(first_user_msg, response)
                st.session_state.chat.title = new_title
                st.session_state.chat._save_history()
            st.rerun()
