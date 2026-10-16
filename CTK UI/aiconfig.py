import json
import os
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen
API_URL = "https://ai.hackclub.com/proxy/v1/chat/completions"
DEFAULTMODEL = "qwen/qwen3-32b"
class HackAIError(RuntimeError):
    pass
def parseairesponse(content):
    text = str(content or "").strip()
    if text.startswith("```"):
        lines = text.splitlines()
        lines = lines[1:]
        if lines and lines[-1].strip() == "```":
            lines.pop()
        text = "\n".join(lines).strip()
    start = text.find("{")
    end = text.rfind("}")
    if start == -1 or end == -1:
        return {
            'reply': text,
            'actions': []
        }
    try:
        result = json.loads(text[start:end + 1])
        if not isinstance(result, dict):
            return {"reply": text, "actions": []}
        reply = result.get("reply", "")
        actions = result.get("actions", [])
    except (json.JSONDecodeError, TypeError):
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
            'role': "system",
            'content': (
                "You are Armor AI, the concise assistant inside Armor3D. "
                "The live scene later in this request is the only authoritative drawing state. "
                "Never claim that an object exists unless it appears in that live scene. "
                "Never recover objects or coordinates from chat history. "
                "When the user asks you to draw something, you may return drawing actions. "
                "For now, the only permitted action is draw_line. "
                "Coordinates use [x, y, z], y is up, and the drawing plane is X/Z. "
                "One model unit currently equals one inch. "
                'A line action uses {"action":"draw_line",'
                '"start":[0,0,0],"end":[5,0,0]}. '
                "Always respond using one valid JSON object: "
                '{"reply":"short_response","actions":[]}. '
                "You may use Markdown inside reply, but do not wrap the JSON in Markdown."
            ),
        }
    ]
    for saved_message in (history or [])[-12:]:
        if saved_message.get("role") == 'user':
            messages.append({
                "role": "user",
                "content": str(
                    saved_message.get("content", "")
                )[:2000]
            })
    messages.append({
        "role": "system",
        'content': (
            "This is the current live Armor3D scene. "
            "It replaces every drawing description from previous messages. "
            "If objects is empty, use that fact only when the user asks about the "
            "drawing or scene. For greetings and unrelated questions, answer normally "
            "without mentioning whether the scene is empty. "
            "Never display this raw JSON.\n"
            f"{scene_text}"
        ),
    })
    messages.append({
        'role':"user",
        "content": message
    })
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
