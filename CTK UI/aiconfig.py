import json
import os
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen
API_URL = "https://ai.hackclub.com/proxy/v1/chat/completions"
DEFAULTMODEL = "qwen/qwen3-32b"
class HackAIError(RuntimeError):
    pass
def parseairesponse(content):
    text= str(content).strip()
    if text.startswith("'''"):
        lines = text.splitlines()
        if lines:
            lines.pop()
        if lines and lines[-1].strip() == "'''":
            lines.pop()
        text = '\n'.join(lines).strip()
    start = text.find("{")
    end = text.rfind("}")
    if start == -1 or end == -1:
        return {
            'reply': text,
            'actions': []
        }
    try:
        result = json.loads(text[start:end + 1])
    except json.JSONDecodeError:
        return {
            'reply': text,
            "actions": [],
        }
    if not isinstance(reply, str):
        reply = str(reply)
    if not isinstance(actions, list):
        actions = []
    return {
        'reply': reply,
        'actions': actions
    }
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
                "You are Armor AI, the concise assistant inside Armor3D. "
                "The current scene below is refreshed for every request and is "
                "the authoritative state of the drawing. Chat history may describe "
                "an older scene; ignore those older claims whenever they conflict "
                "with the current scene. Objects marked in_progress are currently "
                "being drawn and may include a moving preview endpoint. Use the "
                "coordinates, measurements, color_name, selection, and closed state to "
                "answer questions. Do not invent objects. Treat the scene as private "
                "application state and never print, quote, or mention its raw JSON. "
                "Do not list vertices or recalculate dimensions unless requested. "
                "When the user asks you to draw something, you may return drawing actions"
                "For now, the only allowed action is draw_line"
                "A line action must use this structure and nothing else:"
                '{"action":"draw_line","start":[0,0,0],"end":[5,0,0]}. '
                "Always respond using only one valid JSON object with this structure:"
                '{"reply":"short response to user","actions":[]}'
                "Do not wrap the json in a Markdown code block"
                "Answer naturally in English.\n"
                f"CURRENT ARMOR3D SCENE:\n{scene_text}"
            ),
        }
    ]
    for saved_message in (history or [])[-24:]:
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
        content = data["choices"][0]["message"]["content"]
        return parseairesponse(content)
    except (KeyError, IndexError, TypeError) as error:
        raise HackAIError(
            f"Unexpected HackAI response: {data}"
        ) from error
