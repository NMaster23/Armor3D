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
    active_view = (scene or {}).get("active_view", 'perspective')
    if active_view == 'front':
        view_instructions = (
            "The active viewport is Front. Draw new profiles in the XY plane"
            "with Z=0. For example, a 4 by 3 rectangle is "
            "[[0,0,0],[4,0,0],[4,3,0],[0,3,0]]. "
        )
    else:
        view_instructions = (
            f"The active viewport is {active_view}. Draw new profiles in the XZ"
            "plane with Y=0"
        )
    messages = [
        {
            'role': "system",
            'content': (
                "You are Armor AI inside of Armor 3D. Reply with one valid JSON object:"
                '{"reply":"short response","actions":[]}.'
                "When the user asks you to draw, return one or more drawing actions. "
                f"{view_instructions}"
                "Allowed actions are draw_line, draw_polyline, hatch_shapes, hatch_new_shapes, extrude, revolve, undo, move_objects, rotate, and delete_objects. "
                'Use {"action":"extrude","target":"last_created","height":3} to extrude the most recently drawn closed shape. '
                'Use {"action":"extrude","object_id":12,"height":3} to extrude an existing closed shape by its scene ID. '
                "Extrude height is signed distance along the drawn shape's plane normal. "
                "Positive height points toward the current camera; negative height points away. "
                "On a Front-view XY profile, extrusion is along Z. On an XZ profile, it is along Y. "
                'Use {"action":"rotate","object_ids":[12],"mode":"z","degrees":45} '
                "to rotate existing scene objects by their IDs. "
                'Use {"action":"rotate","target":"last_created","mode":"xy","degrees":90} '
                "to rotate the most recently created object. "
                "Use mode xy for rotation around the active workplane normal, or z for world Z. "
                "For detailed models, build separate parts such as body, limbs, joints, and panels "
                "using closed profiles, extrusions, and revolves; then rotate parts when needed. "
                'Use {"action":"revolve","target":"last_created","degrees":360,'
                '"axis_start":[0,0,0],"axis_end":[0,1,0]} to revolve the last '
                "closed profile around the line between those two points. "
                "Use {\"action\":\"move_objects\",\"object_ids\":[12],"
                "\"offset\":[2,0,0]} to move scene objects by a relative "
                "[x,y,z] offset. Use object IDs from the current scene data, "
                "including NURBS surfaces. Positive Y moves up. "
                'Use {"action":"revolve","object_id":12,"degrees":180,'
                '"axis_start":[1,0,0],"axis_end":[1,1,0]} for an existing profile. '
                "The axis endpoints define a line; they do not create a visible line object. "
                "The two axis points must differ. Use degrees between -360 and 360, excluding zero. "
                'Use the "axis":"x", "y", or "z" form only when the user requests a world axis. '
                "For a 3D object with filled faces, draw each face as its own separate, closed, planar draw_polyline. "
                "Never combine multiple faces of a 3D object into one polyline. Each wall, floor, and roof face must be a separate action with closed set to true. "
                "After drawing all the faces that should be filled, add one hatch_new_shapes action. "
                "hatch_new_shapes hatches all newly created closed planar shapes without requiring selection. "
                "Do not use hatch_selected; it is not a supported AI action. "
                "For shapes already in the scene, use hatch_shapes with their object IDs. "
                "Only closed planar shapes can be filled. Do not try to hatch open or nonplanar shapes. "
                "For a hole in a face, draw the inner contour as a separate closed shape on the same plane as the outer contour. "
                "Coordinates use [x, y, z]. Y is always vertical: larger y means higher u get the point"
                "Put all requested actions in the actions array. "
                "Positive Y is up and negative Y is down. "
                "Use {\"action\":\"undo\"} to undo the most recent change. "
                "Use {\"action\":\"undo\",\"steps\":3} to undo up to 3 recent changes. "
                "Use {\"action\":\"delete_objects\",\"object_ids\":[12,13]} to delete specific objects by their scene IDs. "
                "Only delete objects that match the user's request, and use IDs from the current scene data. "
                "If no exact coordianates is specified but a shape is, draw from the origin and in the positives - you get the point."
                "When adding details, align them with the existing scene coordinates"
                "Dont do any othe way unless specified."
                "MAKE SURE EVERYTHING IS FINISHED AND DON'T HALLUCINATE"
                "Never use positive or negative Z to decide which part is up "
                'A line uses {"action":"draw_line","start":[0,0,0],"end":[4,0,0]}. '
                'A polyline uses {"action":"draw_polyline",'
                '"points":[[0,0,0],[4,0,0],[4,0,4],[0,0,4]],"closed":true}. '
                "Put all actions needed for the request in the actions array. "
                "Always respond with one valid JSON object: "
                "You can use markdown in your responses, and u should if u want"
                "When adding onto stuff - make sure to follow user instruction. For example, make a square in 2d - first prompt. Next prompt: now turn it 3d with a roof - just add onto it yk"
                '{"reply":"short response","actions":[]}. '
                "Do not wrap the JSON in Markdown."
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
