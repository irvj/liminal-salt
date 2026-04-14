from django.apps import AppConfig


class ChatConfig(AppConfig):
    name = 'chat'

    def ready(self):
        import sys
        import os
        from django.conf import settings as django_settings

        # Don't run for management commands
        management_cmds = ('check', 'migrate', 'collectstatic', 'makemigrations', 'shell', 'test')
        if len(sys.argv) > 1 and sys.argv[1] in management_cmds:
            return

        # Don't start in runserver's reloader parent process
        if 'runserver' in sys.argv and os.environ.get('RUN_MAIN') != 'true':
            return

        # Seed default personas if they don't exist yet
        from .services.context_manager import ensure_default_personas
        ensure_default_personas(django_settings.PERSONAS_DIR)

        from .services.memory_worker import start_scheduler
        start_scheduler()
