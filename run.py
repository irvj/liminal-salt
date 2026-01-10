#!/usr/bin/env python3
"""
Simple launcher for Liminal Salt.
Creates virtual environment and installs dependencies on first run.
"""
import subprocess
import sys
from pathlib import Path

venv = Path(".venv")
pip = venv / ("Scripts/pip" if sys.platform == "win32" else "bin/pip")
python = venv / ("Scripts/python" if sys.platform == "win32" else "bin/python")

if not venv.exists():
    print("Creating virtual environment...")
    subprocess.run([sys.executable, "-m", "venv", ".venv"], check=True)
    subprocess.run([str(pip), "install", "-q", "--disable-pip-version-check", "-r", "requirements.txt"], check=True)
    print()

url = "http://localhost:8000"
print(f"Starting Liminal Salt at \033]8;;{url}\033\\{url}\033]8;;\033\\")
subprocess.run([str(python), "-c",
    "from waitress import serve; "
    "from liminal_salt.wsgi import application; "
    "serve(application, host='127.0.0.1', port=8000, _quiet=True)"])
