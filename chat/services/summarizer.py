from .llm_client import call_llm


class Summarizer:
    """Title generation service. Memory operations are in memory_manager.py."""

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

        try:
            title = call_llm(
                self.api_key, self.model,
                [{"role": "user", "content": prompt}],
                site_url=self.site_url, site_name=self.site_name
            )

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
