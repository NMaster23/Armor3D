import json
import os
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen
API_URL = "https://ai.hackclub.com/proxy/v1/chat/completions"
DEFAULTMODEL = "openai/gpt-4o-mini"
class HackAIError(RuntimeError):
    pass
def sendmessage(message, api_key=None, model=DEFAULTMODEL):
    api_key = ( api_key or os.getenv("HACKAI_API_KEY") or os.getenv("OPENAI_API_KEY") or "").strip()
    if not api_key:
        raise HackAIError("No HackAI API key is configured.")
    payload = {"model": model, "messages": [
        {
            "role": 'user',
            'content': message
        }
    ]}
    request = Request(API_URL, data=json.dumps(payload).encode("utf-8"), headers={
        "Authorization": f"Bearer {api_key}",
        "Content-Type": 'application/json'
    },
    method = 'POST'
    )
    try: 
        with urlopen(request, timeout=60 ) as response:
            data= json.loads(response.read().decode("utf-8"))
    except HTTPError as error:
        details = error.read().decode("utf-8", errors="replace")
        raise HackAIError(
            f"HackAI returned HTTP {error.code}: {details}"
        ) from error
    except URLError as error:
        raise HackAIError(f"Could not connect to HackAI: {error.reason}") from error
    try:
        return data["choices"][0]["message"]["content"]
    except (KeyError, IndexError, TypeError) as error:
        raise HackAIError(f"Unexpected HackAI response: {data}") from error
    
    