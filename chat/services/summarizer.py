import requests
import json
import os

class Summarizer:
    def __init__(self, api_key, model, site_url=None, site_name=None):
        self.api_key = api_key
        self.model = model
        self.site_url = site_url
        self.site_name = site_name

    def generate_title(self, user_prompt, assistant_response):
        """
        Generate a title based on BOTH user prompt and assistant response.

        Args:
            user_prompt: The first user message
            assistant_response: The first assistant response (can be empty/None/ERROR)

        Returns:
            A clean 2-5 word title
        """
        # Fallback #1: If both are empty, return default
        if not user_prompt and not assistant_response:
            return "New Chat"

        # Fallback #2: If user prompt is empty, return default
        if not user_prompt:
            return "New Chat"

        # Determine if we should use assistant response or just user prompt
        use_response = (
            assistant_response and
            not assistant_response.startswith("ERROR:") and
            len(assistant_response.strip()) > 0
        )

        # Build appropriate prompt for title generation
        if use_response:
            # Both user prompt and assistant response available
            prompt = (
                "Generate a very short, 2-5 word title for a chat session.\n"
                "Rules:\n"
                "- NO quotes, punctuation, or special characters\n"
                "- NO model tokens like [INST], </s>, <s>\n"
                "- Just the plain title text\n"
                "- Be descriptive but concise\n\n"
                f"USER ASKED: {user_prompt[:200]}\n"
                f"ASSISTANT REPLIED: {assistant_response[:200]}\n\n"
                "TITLE:"
            )
        else:
            # Only user prompt available (empty response case)
            prompt = (
                "Generate a very short, 2-5 word title that captures the essence of this question.\n"
                "Rules:\n"
                "- NO quotes, punctuation, or special characters\n"
                "- NO model tokens\n"
                "- Just the plain title text\n\n"
                f"USER QUESTION: {user_prompt[:200]}\n\n"
                "TITLE:"
            )

        headers = {
            "Authorization": f"Bearer {self.api_key}",
            "Content-Type": "application/json"
        }
        if self.site_url:
            headers["HTTP-Referer"] = self.site_url
        if self.site_name:
            headers["X-Title"] = self.site_name

        payload = {
            "model": self.model,
            "messages": [{"role": "user", "content": prompt}]
        }

        try:
            response = requests.post(
                url="https://openrouter.ai/api/v1/chat/completions",
                headers=headers,
                data=json.dumps(payload),
                timeout=30
            )
            response.raise_for_status()
            data = response.json()
            title = data['choices'][0]['message']['content'].strip()

            # Clean up title aggressively
            title = self._clean_title(title)

            # Validation: If title is too long, too short, or contains artifacts, use fallback
            if not title or len(title) < 3 or len(title) > 50 or self._has_artifacts(title):
                return user_prompt[:50] + ("..." if len(user_prompt) > 50 else "")

            return title
        except Exception:
            # Fallback #3: On any error, use truncated user prompt
            return user_prompt[:50] + ("..." if len(user_prompt) > 50 else "")

    def _clean_title(self, title):
        """Remove all known model artifacts and formatting"""
        artifacts = ['<s>', '</s>', '[INST]', '[/INST]', '<<SYS>>', '<</SYS>>', '###', 'Prompt']
        for artifact in artifacts:
            title = title.replace(artifact, '')

        # Remove quotes and excessive punctuation
        title = title.replace('"', '').replace("'", '').strip()
        title = title.rstrip('.:;,!?')

        # Remove leading/trailing whitespace and newlines
        title = ' '.join(title.split())

        return title

    def _has_artifacts(self, title):
        """Check if title contains common model artifacts"""
        bad_patterns = ['[', ']', '<', '>', '#', '\n', 'Prompt', 'INST', 'SYS']
        return any(pattern in title for pattern in bad_patterns)

    def update_long_term_memory(self, messages, ltm_file="long_term_memory.md"):
        if not messages:
            return

        # Load existing LTM
        existing_ltm = ""
        if os.path.exists(ltm_file):
            with open(ltm_file, 'r') as f:
                existing_ltm = f.read()

        # Format the new conversation - ONLY USER MESSAGES
        conversation_text = ""
        for msg in messages:
            if msg['role'] == 'user':
                conversation_text += f"User: {msg['content']}\n"

        # Check if old format needs migration
        migration_note = ""
        if existing_ltm and ("# Facts & Knowledge Base" in existing_ltm or "- **" in existing_ltm):
            # Old format detected, add migration hint to prompt
            migration_note = (
                "MIGRATION NOTE: The existing memory uses an old format with either a 'Facts & Knowledge Base' section "
                "or bullet list formatting. Please reorganize into the new 3-section NARRATIVE format:\n"
                "1. Remove the 'Facts & Knowledge Base' section - only keep knowledge that's specifically about the USER\n"
                "2. Convert ALL bullet lists to flowing narrative prose paragraphs\n"
                "3. Reframe content to describe the USER, not general facts\n"
                "4. Write as if briefing another AI about who this person is\n\n"
            )

        prompt = (
            migration_note +
            "You are an advanced memory management system for an AI. "
            "Your task is to maintain a living 'Long Term Memory' document that describes the USER as a person "
            "in natural narrative prose to help the AI understand who they're talking to.\n\n"
            "EXISTING LONG TERM MEMORY:\n"
            f"{existing_ltm if existing_ltm else 'None'}\n\n"
            "NEW CONVERSATION:\n"
            f"{conversation_text}\n\n"

            "NOTE: The conversation below contains ONLY User messages. "
            "There are no Assistant messages included.\n\n"

            "INSTRUCTIONS:\n\n"

            "## 1. USER PROFILE (FACTUAL AND INFORMATIVE)\n"
            "Write a section called '# User Profile'. Be direct, specific, and factual.\n"
            "Format: Write in clear, informative prose based on evidence from user messages.\n\n"

            "⚠️ CRITICAL - PERSPECTIVE:\n"
            "Write from a NEUTRAL THIRD-PERSON perspective, as an objective briefing document.\n"
            "DO NOT use first-person references from your perspective as the memory LLM:\n"
            "❌ WRONG: 'They built a chat interface to communicate with me'\n"
            "❌ WRONG: 'They're using this application to talk to me'\n"
            "❌ WRONG: 'In their interactions with me, they...'\n"
            "✅ CORRECT: 'They built a chat interface frontend for an LLM'\n"
            "✅ CORRECT: 'They are developing a Python-based chat application'\n"
            "✅ CORRECT: 'They have discussed gameplay systems and C++ development'\n\n"

            "Include ONLY information the user has explicitly shared:\n"
            "- Specific facts about their work, role, company, tech stack\n"
            "- Technical skills and knowledge areas they've demonstrated\n"
            "- Actual projects or problems they've discussed\n"
            "- Communication patterns observed in their writing\n"
            "- Preferences they've explicitly stated\n\n"

            "DO NOT:\n"
            "❌ Infer personality traits without clear evidence\n"
            "❌ Use flowery or elaborate language\n"
            "❌ Make assumptions about what they might be like\n"
            "❌ Write narrative descriptions - stick to facts\n"
            "❌ Use first-person perspective ('with me', 'to me', 'this AI')\n\n"

            "BE SPECIFIC:\n"
            "✅ 'Works as a senior engineer at X company, focuses on Y technology'\n"
            "✅ 'Has asked detailed questions about Z, indicating deep knowledge'\n"
            "✅ 'Prefers concise explanations, often asks follow-up questions'\n\n"

            "AVOID VAGUE DESCRIPTIONS:\n"
            "❌ 'Approaches conversations with curiosity and thoughtfulness'\n"
            "❌ 'Has an analytical mindset and values precision'\n"
            "❌ 'Tends to think deeply about technical problems'\n\n"

            "EVOLVE this section with new insights, but NEVER remove established facts unless contradicted.\n\n"

            "## 2. CRITICAL PERSONAL FACTS (PERMANENT ANCHORS - NARRATIVE)\n"
            "Write a section called '# Critical Personal Facts'. This is the SECOND PRIORITY.\n"
            "Format: Write in NATURAL PROSE describing the permanent facts about their life.\n\n"

            "Include:\n"
            "- Family members, close friends, important relationships (names and context)\n"
            "- Where they live, what they do for work, their educational background\n"
            "- Stated life goals, dreams, or plans they've shared\n"
            "- Health considerations, allergies, or chronic conditions\n"
            "- Strong preferences or values they've explicitly stated\n\n"

            "Write as narrative: 'The user has a brother named Tom who...' not 'Brother: Tom'\n"
            "RULES: Once added, NEVER remove unless user explicitly contradicts it.\n\n"

            "## 3. LIVING INTERESTS & KNOWLEDGE (FLUID - NARRATIVE)\n"
            "Write a section called '# Living Interests & Knowledge'. This EVOLVES DYNAMICALLY.\n"
            "Format: Write in NATURAL PROSE about what currently engages them.\n\n"

            "Describe:\n"
            "- Hobbies and interests they're currently passionate about\n"
            "- Technical domains, skills, or knowledge they're building\n"
            "- Media, books, games, shows they're engaged with\n"
            "- Topics they enjoy discussing or learning about\n\n"

            "ONLY include knowledge/facts that are ABOUT THE USER (their preferences, their experiences, their expertise).\n"
            "DO NOT include general facts (like 'Apollo 11 landed on the moon') unless it's about the USER'S "
            "relationship to that fact (e.g., 'The user has a deep interest in space history and often references Apollo 11')\n\n"

            "Write narratively: 'The user has recently been exploring mechanical keyboards, particularly...' "
            "not 'Keyboards: Topre switches...'\n\n"

            "DEPRECATION: If an interest hasn't been reinforced over 2-3 updates, you may remove it. "
            "Keep interests mentioned multiple times. Give new interests time before deprecating.\n\n"

            "## 4. FORMATTING RULES\n"
            "- Write in flowing narrative paragraphs, NOT bullet lists\n"
            "- Use natural sentences: 'The user is...' 'They tend to...' 'They have...' 'They enjoy...'\n"
            "- Group related information into cohesive paragraphs\n"
            "- NO timestamps, meta-commentary, or conversational filler\n"
            "- Make it read like a character description or briefing document\n"
            "- Be specific and useful for an AI to understand who they're talking to\n\n"

            "## 5. CRITICAL PRESERVATION CHECK\n"
            "Before removing ANY fact from 'Critical Personal Facts':\n"
            "- Would the user be surprised or upset if this was gone?\n"
            "- Is this a core part of their identity?\n"
            "- Did they share this with emphasis or emotional weight?\n"
            "If YES to any, KEEP IT.\n\n"

            "## 6. WHAT NOT TO INCLUDE\n"
            "DO NOT include general knowledge or facts unless they're specifically about the USER:\n"
            "❌ 'Apple Inc is a public company with stock ticker AAPL' (general fact)\n"
            "✅ 'The user is deeply invested in the Apple ecosystem and follows company news closely' (about user)\n"
            "❌ 'Rush wrote Spirit of Radio in 1980' (general fact)\n"
            "✅ 'The user is a longtime Rush fan and often references their lyrics in conversation' (about user)\n\n"

            "FOCUS: This is a profile of the USER, not an encyclopedia. Describe the person, not the world."
        )

        headers = {
            "Authorization": f"Bearer {self.api_key}",
            "Content-Type": "application/json"
        }
        if self.site_url:
            headers["HTTP-Referer"] = self.site_url
        if self.site_name:
            headers["X-Title"] = self.site_name

        payload = {
            "model": self.model,
            "messages": [{"role": "user", "content": prompt}]
        }

        try:
            response = requests.post(
                url="https://openrouter.ai/api/v1/chat/completions",
                headers=headers,
                data=json.dumps(payload),
                timeout=30
            )
            response.raise_for_status()
            data = response.json()
            updated_ltm = data['choices'][0]['message']['content'].strip()

            # Clean up tokens
            updated_ltm = updated_ltm.replace('<s>', '').replace('</s>', '').strip()
            
            # Safety check
            if len(updated_ltm) < 10 and len(existing_ltm) > 50:
                return

            # Write back to file
            with open(ltm_file, 'w') as f:
                f.write(updated_ltm)
            print(f"Long-term memory and User Profile updated in {ltm_file}")
            
        except Exception as e:
            print(f"Error updating long term memory: {e}")

    def modify_memory_with_command(self, command, ltm_file="long_term_memory.md"):
        """
        Modify the long-term memory based on a user command.

        Args:
            command: User's instruction (e.g., "Forget my brother Tom's name")
            ltm_file: Path to the long-term memory file

        Returns:
            The updated memory content, or None on error
        """
        # Load existing memory
        existing_ltm = ""
        if os.path.exists(ltm_file):
            with open(ltm_file, 'r') as f:
                existing_ltm = f.read()

        if not existing_ltm:
            return None

        prompt = (
            "You are a memory management system. The user has requested a modification to their stored memory profile.\n\n"
            f"USER'S COMMAND: {command}\n\n"
            "CURRENT MEMORY:\n"
            f"{existing_ltm}\n\n"
            "INSTRUCTIONS:\n"
            "Apply the user's command to modify the memory. Return the complete updated memory.\n"
            "- If asked to 'forget' something, remove that information entirely\n"
            "- If asked to 'update' or 'change' something, modify that information\n"
            "- If asked to 'add' something, include the new information in the appropriate section\n"
            "- Preserve all other information that wasn't explicitly mentioned\n"
            "- Maintain the same markdown format and section structure (# User Profile, # Critical Personal Facts, # Living Interests & Knowledge)\n"
            "- Write in natural narrative prose, not bullet lists\n\n"
            "Return ONLY the updated memory content, no explanations or commentary."
        )

        headers = {
            "Authorization": f"Bearer {self.api_key}",
            "Content-Type": "application/json"
        }
        if self.site_url:
            headers["HTTP-Referer"] = self.site_url
        if self.site_name:
            headers["X-Title"] = self.site_name

        payload = {
            "model": self.model,
            "messages": [{"role": "user", "content": prompt}]
        }

        try:
            response = requests.post(
                url="https://openrouter.ai/api/v1/chat/completions",
                headers=headers,
                data=json.dumps(payload),
                timeout=30
            )
            response.raise_for_status()
            data = response.json()
            updated_ltm = data['choices'][0]['message']['content'].strip()

            # Clean up tokens
            updated_ltm = updated_ltm.replace('<s>', '').replace('</s>', '').strip()

            # Safety check - don't accept empty or too-short responses
            if len(updated_ltm) < 10:
                return None

            # Write back to file
            with open(ltm_file, 'w') as f:
                f.write(updated_ltm)

            return updated_ltm

        except Exception as e:
            print(f"Error modifying memory: {e}")
            return None
