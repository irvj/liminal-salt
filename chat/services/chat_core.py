import requests
import json
import os
import time
from datetime import datetime, timezone
from zoneinfo import ZoneInfo

class ChatCore:
    def __init__(self, api_key, model, site_url=None, site_name=None, system_prompt="", context_history_limit=50, history_file=None, persona="assistant", user_timezone="UTC", assistant_timezone=None):
        self.api_key = api_key
        self.model = model
        self.site_url = site_url
        self.site_name = site_name
        self.system_prompt = system_prompt
        self.context_history_limit = context_history_limit
        self.history_file = history_file
        self.persona = persona
        self.user_timezone = user_timezone
        self.assistant_timezone = assistant_timezone
        self.title = "New Chat"
        self.messages = self._load_history()

    def _load_history(self):
        if self.history_file and os.path.exists(self.history_file):
            try:
                with open(self.history_file, 'r') as f:
                    data = json.load(f)
                    if isinstance(data, dict):
                        self.title = data.get("title", "New Chat")
                        self.persona = data.get("persona", self.persona)
                        return data.get("messages", [])
                    return data
            except Exception:
                return []
        return []

    def _save_history(self):
        if not self.history_file:
            return
        try:
            to_save = {
                "title": self.title,
                "persona": self.persona,
                "messages": self.messages  # Save ALL messages locally
            }
            with open(self.history_file, 'w') as f:
                json.dump(to_save, f, indent=4)
        except Exception as e:
            print(f"Error saving history: {e}")

    def clear_history(self):
        self.messages = []
        if self.history_file and os.path.exists(self.history_file):
            os.remove(self.history_file)

    def _get_payload_messages(self):
        payload = []

        # Build system prompt with current local time PREPENDED for highest attention
        if self.system_prompt:
            now_utc = datetime.now(timezone.utc)

            # User's local time
            try:
                user_tz = ZoneInfo(self.user_timezone)
                user_local = now_utc.astimezone(user_tz)
                user_time_str = user_local.strftime("%A, %B %d, %Y at %I:%M %p")
            except Exception:
                user_time_str = now_utc.strftime("%A, %B %d, %Y at %I:%M %p UTC")

            # Build time context with explicit instructions to USE the provided time
            time_instruction = "When asked about or considering the time, use the time above. This time is accurate and updated with each message. Do not guess, assume, or make up times. Do not say you lack real-time access — you are being given the current time."

            if self.assistant_timezone and self.assistant_timezone != self.user_timezone:
                try:
                    asst_tz = ZoneInfo(self.assistant_timezone)
                    asst_local = now_utc.astimezone(asst_tz)
                    asst_time_str = asst_local.strftime("%A, %B %d, %Y at %I:%M %p")
                    time_instruction = "When asked about or considering the time, use the times above. These are accurate and updated with each message. Do not guess, assume, or make up times. Do not say you lack real-time access — you are being given the current time."
                    time_context = f"*** CURRENT TIME ***\nUser's time: {user_time_str}\nYour time: {asst_time_str}\n\n{time_instruction}\n\n"
                except Exception:
                    time_context = f"*** CURRENT TIME: {user_time_str} ***\n{time_instruction}\n\n"
            else:
                time_context = f"*** CURRENT TIME: {user_time_str} ***\n{time_instruction}\n\n"

            # PREPEND time context for highest transformer attention
            payload.append({"role": "system", "content": time_context + self.system_prompt})

        window_size = self.context_history_limit * 2
        recent_messages = self.messages[-window_size:]

        # Add messages without timestamp prefixes (timestamps stored separately for UI)
        for msg in recent_messages:
            payload.append({"role": msg["role"], "content": msg["content"]})

        return payload

    def send_message(self, user_input, skip_user_save=False):
        # Only add user message if not already saved (e.g., by start_chat)
        if not skip_user_save:
            timestamp = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
            self.messages.append({"role": "user", "content": user_input, "timestamp": timestamp})

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
            "messages": self._get_payload_messages()
        }

        # Retry logic: Try up to 2 times if we get empty responses
        max_retries = 2
        assistant_message = None
        last_error = None

        for attempt in range(max_retries):
            if attempt > 0:
                print(f"[ChatCore] Retry attempt {attempt + 1}/{max_retries} due to empty response...")
                # Add delay before retry to give the API time to process
                time.sleep(2)

            try:
                response = requests.post(
                    url="https://openrouter.ai/api/v1/chat/completions",
                    headers=headers,
                    data=json.dumps(payload),
                    timeout=120
                )
                response.raise_for_status()
                data = response.json()

                if 'choices' not in data or not data['choices']:
                    last_error = "No response content from API."
                    print(f"[ChatCore] Attempt {attempt + 1} failed: No choices in response")
                    continue  # Retry

                assistant_message = data['choices'][0]['message']['content']

                # Clean up tokens
                assistant_message = assistant_message.replace('<s>', '').replace('</s>', '').strip()

                if assistant_message:
                    # Success! We have a valid response
                    if attempt > 0:
                        print(f"[ChatCore] Retry successful on attempt {attempt + 1}")
                    break
                else:
                    last_error = "The model returned an empty response."
                    print(f"[ChatCore] Attempt {attempt + 1} failed: Empty response after cleanup")
                    # Continue to retry if we have attempts left

            except Exception as e:
                last_error = str(e)
                print(f"[ChatCore] Attempt {attempt + 1} failed with exception: {last_error}")
                # Continue to retry if we have attempts left

        # After all retries, check if we got a valid response
        if not assistant_message:
            error_msg = f"ERROR: {last_error}"
            if max_retries > 1:
                error_msg += f" (tried {max_retries} times)"
            return error_msg

        assistant_timestamp = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
        self.messages.append({"role": "assistant", "content": assistant_message, "timestamp": assistant_timestamp})
        self._save_history()
        return assistant_message
