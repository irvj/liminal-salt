import json
import os

def load_config(path="config.json"):
    if not os.path.exists(path):
        return {}
    with open(path, 'r') as f:
        return json.load(f)
