from django.apps import AppConfig


class ChatConfig(AppConfig):
    name = 'chat'

    def ready(self):
        import sys
        import os

        # Don't start scheduler for management commands
        management_cmds = ('check', 'migrate', 'collectstatic', 'makemigrations', 'shell', 'test')
        if len(sys.argv) > 1 and sys.argv[1] in management_cmds:
            return

        # Don't start in runserver's reloader parent process
        if 'runserver' in sys.argv and os.environ.get('RUN_MAIN') != 'true':
            return

        from .services.memory_worker import start_scheduler
        start_scheduler()
