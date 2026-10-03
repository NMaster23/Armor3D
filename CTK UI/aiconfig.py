import json
import os
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen
API_URL = "https://ai.hackclub.com/proxy/v1/chat/completions"
DEFAULTMODEL = 'openai/gpt-6-sol-pro'
class HackAIError(RuntimeError):
    pass
def sendmessage(
        message, api_key=None,
        model=DEFAULTMODEL, history=None, scene=None
):
    api_key = (api_key 
               or os.getenv("HACKAI_API_KEY")
                or os.getenv("OPENAI_API_KEY")
                or ""
        ).strip()
    if not api_key:
        raise HackAIError("No HackAI API key is configured.")
    scene_text = json.dumps(
        scene or {"objects": []},
        separators=(",", ":")
    )
    messages = [
        {
            "role": "system",
            "content": (
                "You are Armor AI, the assistant inside Armor3D "
                "Give concise and helpful answers  "
                "You receive structured information about the current drawing. "
                "Use the object coordinates, colors, selected state, and closed "
                "state to answer questions about the drawing. Also, colors are green, gold, brown, and white btw answer accordingly"
                "Do not invent objects that are not in the scene.\n\n"
                f"CURRENT ARMOR3D SCENE:\n{scene_text}"
            ),
        }
    ]
    for saved_message in (history or [])[-12:]:
        if saved_message.get("role") in ("user", "assistant"):
            messages.append({
                "role": saved_message['role'],
                'content': str(saved_message.get("content", "")[:4000]) })
    messages.append({
        "role": "user",
        "content": message, })
    payload = {
        "model": model,
        "messages": messages }
    request = Request(
        API_URL,
        data=json.dumps(payload).encode("utf-8"),
        headers={
            "Authorization": f"Bearer {api_key}",
            "Content-Type": "application/json",},
        method="POST",)
    try:
        with urlopen(request, timeout=60) as response:
            data = json.loads(response.read().decode("utf-8"))
    except HTTPError as error:
        details = error.read().decode(
            "utf-8",
            errors="replace")
        raise HackAIError(
            f"HackAI returned HTTP {error.code}: {details}"
        ) from error

    except URLError as error:
        raise HackAIError(
            f"Could not connect to HackAI: {error.reason}"
        ) from error
    try:
        return data["choices"][0]["message"]["content"]
    except (KeyError, IndexError, TypeError) as error:
        raise HackAIError(
            f"Unexpected HackAI response: {data}"
        ) from error
        