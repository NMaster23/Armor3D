import customtkinter as ctk
from ctypes import windll
from tkinter import Canvas, Frame, filedialog, Toplevel, StringVar
import sys
from PIL import Image, ImageEnhance, ImageTk, ImageDraw, ImageGrab
from pathlib import Path
import os
from armor_core import ViewportRenderer
ctk.set_appearance_mode('dark')
def getpath(relativepath):
    try:
        basepath = sys._MEIPASS
    except AttributeError:
        basepath = os.path.abspath(".")
    return os.path.join(basepath, relativepath)
FR_PRIVATE = 0x10
def loadfont(fontpath):
    windll.gdi32.AddFontResourceExW(fontpath, FR_PRIVATE, 0)
loadfont(getpath("Assets/Lexend-VariableFont_wght.ttf"))
loadfont(getpath("Assets/Iceland-Regular.ttf"))
app = ctk.CTk()
from tkinter import font
app.title("Armor 3D")
app.geometry("1100x700")
app.minsize(850, 560)
canvas = Canvas(app, bg="#242B23", highlightthickness=0)
canvas.pack(fill='both', expand=True)
command = ctk.CTkEntry(canvas, placeholder_text="Command:", font=("Lexend", 12), fg_color="#3B322A", border_color="#70543B")
history = ctk.CTkTextbox( canvas, font=("Lexend", 11), fg_color="#3B322A", border_color="#70543B",  border_width=2)
history_window = canvas.create_window(8, 19, window=history, anchor='nw', height=45)
history.configure(state="disabled")
command_window = canvas.create_window(8, 70, window=command, anchor='nw', height=20)
horizontal = canvas.create_line(100, 100, 1200, 100, fill="#70543B", width=3)
vertical = canvas.create_line(100, 100, 100, 850, fill="#70543B", width=3)
viewport = Frame(canvas, bg="#101010", bd=0, highlightthickness=0, takefocus=1)
viewport_window = canvas.create_window(101,  101,  window=viewport, anchor="nw", width=700, height=500,)
renderer = None
def initialize_renderer():
    global renderer
    app.update_idletasks()
    renderer = ViewportRenderer(viewport.winfo_id(),  max(1, viewport.winfo_width()), max(1, viewport.winfo_height()),)
    render_frame()
def render_frame():
    if renderer is not None:
        renderer.render()
        app.after(16, render_frame)
def resize_viewport(event):
    if renderer is not None:
        renderer.resize(max(1, event.width), max(1, event.height))
def viewport_mouse_move(event):
    if renderer is not None:
        renderer.mouse_move(event.x, event.y)
def viewport_mouse_down(event):
    viewport.focus_set()
    if renderer is not None:
        renderer.mouse_button(True)
def viewport_mouse_up(event):
    if renderer is not None:
        renderer.mouse_button(False)
last_right_drag = None
def viewport_right_down(event):
    global last_right_drag, rightpresspos, rightpresstime, rightdragged
    viewport.focus_set()
    last_right_drag = (event.x, event.y)
    rightpresspos = (event.x, event.y)
    rightpresstime = event.time
    rightdragged= False
def viewport_right_drag(event):
    global last_right_drag, rightdragged
    if last_right_drag is None:
        return
    if rightpresspos is not None:
        total_x = event.x-rightpresspos[0]
        total_y = event.y - rightpresspos[1]
        if total_x * total_x + total_y * total_y > 25:
            rightdragged = True
    if renderer is not None:
        dx  = event.x- last_right_drag[0]
        dy = event.y - last_right_drag[1]
        if event.state & 0x0001:
            renderer.orbit(dx, dy)
        else:
            renderer.pan(dx, dy)
    last_right_drag = (event.x, event.y)
def viewport_right_up(event):
    global last_right_drag, rightpresspos, rightdragged
    clicktime = event.time - rightpresstime
    if not rightdragged and clicktime < 350:
        if activecommand is not None:
            cancelactivecommand()
        elif lastcommand is not None:
            repeatlastcommand()
    last_right_drag = None
    rightpresspos = None
    rightdragged = False
def viewport_wheel(event):
    if renderer is not None:
        renderer.zoom(event.delta / 120)
def viewport_key(event, pressed):
    if event.keysym in ("2", "K_2", 'bracketright'):
        return
    if renderer is not None:
        renderer.key_event(event.keysym, pressed)
def viewport_focus_out(event):
    if renderer is not None:
        for key in ("w", "a", "s", "d", "Up", "Down", "Left", "Right"):
            renderer.key_event(key, False)
viewport.bind("<Configure>", resize_viewport)
viewport.bind("<Motion>", viewport_mouse_move)
viewport.bind("<ButtonPress-1>", viewport_mouse_down)
viewport.bind("<ButtonRelease-1>", viewport_mouse_up)
viewport.bind("<ButtonPress-3>", viewport_right_down)
viewport.bind("<B3-Motion>", viewport_right_drag)
viewport.bind("<ButtonRelease-3>", viewport_right_up)
viewport.bind("<MouseWheel>", viewport_wheel)
viewport.bind("<KeyPress>", lambda event: viewport_key(event, True))
viewport.bind("<KeyRelease>", lambda event: viewport_key(event, False))
viewport.bind("<FocusOut>", viewport_focus_out)

shadow_lines= [canvas.create_line(0, 0, 0, 0, fill=color, width=2,  smooth=True, splinesteps=20, state="hidden") for color in ("#283328", "#1D281F", "#152019")]
current_offset = 16
panel_ratio = 0.5
animating=False
def update_shadow(width, height, offset=16):
    left = width *(1-panel_ratio) + offset
    for i, line in enumerate(shadow_lines):
        x = left - 6 + i *2
        canvas.coords(line, x+16, 0, x+5, 5, x, 16, x, height-16, x+5, height-5, x+16, height)

def resize_cmd_boxes(width):
    if sidebar.winfo_manager():
        box_width = min(width-16, width*(1-panel_ratio)+ current_offset-28)
    else:
        box_width = width-16
    canvas.itemconfig(command_window, width=box_width)
    canvas.itemconfig(history_window, width=box_width)

# grid_size = 40
# zoom = 1.0
# pan_x = 0
# pan_y = 0
# last_mouse = None
# def draw_grid(width, height):
#     canvas.delete("viewport_grid")
#     spacing = grid_size * zoom
#     x = 101 + pan_x % spacing
#     while x < width:
#         canvas.create_line(x, 101, x, height, fill="#4C5746", tags="viewport_grid")
#         x += spacing
#     y = 101 + pan_y % spacing
#     while y < height:
#         canvas.create_line(101, y, width, y, fill="#4C5746", tags="viewport_grid")
#         y += spacing
#     canvas.tag_lower("viewport_grid")
# def start_pan(event):
#     global last_mouse
#     if event.x >= 100 and event.y >= 100:
#         last_mouse = (event.x, event.y)
# def move_pan(event):
#     global pan_x, pan_y, last_mouse
#     if last_mouse is None:
#         return
#     pan_x += event.x - last_mouse[0]
#     pan_y += event.y -last_mouse[1]
#     last_mouse = (event.x, event.y)
#     draw_grid(canvas.winfo_width(), canvas.winfo_height())
# def stop_pan(event):
#     global last_mouse
#     last_mouse = None
# canvas.bind("<Button-3>", start_pan)
# canvas.bind("<B3-Motion>", move_pan)
# canvas.bind("<ButtonRelease-3>", stop_pan)
# def zoom_grid(event):
#     global zoom, pan_x, pan_y
#     if event.x < 101 or event.y < 101:
#         return
#     newzoom = max(0.25, min(4.0, zoom * (1.1 if event.delta > 0 else 1 / 1.1)))
#     factor = newzoom / zoom
#     pan_x = (event.x-101) - (event.x - 101 - pan_x) * factor
#     pan_y = (event.y - 101) - (event.y - 101 - pan_y) * factor
#     zoom = newzoom
#     draw_grid(canvas.winfo_width(), canvas.winfo_height())
# canvas.bind("<MouseWheel>", zoom_grid)
snapwindow = None
def resizethings(event):
    canvas.coords(horizontal, 100, 100, event.width, 100)
    canvas.coords(vertical, 100, 100, 100, event.height)
    canvas.itemconfig(viewport_window, width=max(1, event.width - 101), height=max(1, event.height - 101))
    resize_cmd_boxes(event.width)
    update_shadow(event.width, event.height, current_offset)
    if snapwindow is not None:
        canvas.itemconfig(snapwindow, height=max(25, event.height - 515))
    # draw_grid(event.width, event.height)
canvas.bind("<Configure>", resizethings)
past_commands= []
history_index = 0

activecommand = None
lastcommand = None
def writehistory(text):
    history.configure(state='normal')
    history.insert('end', text+ '\n')
    history.see("end")
    history.configure(state='disabled')
def startpolyline(event=None):
    global activecommand
    activecommand = 'polyline'
    command.delete(0, 'end')
    command.configure(placeholder_text = "Start polyline")
    writehistory("> Polyline\nStart polyline")
    viewport.focus_set()
def startcurve(event=None):
    global activecommand
    activecommand = 'curve'
    command.delete(0, 'end')
    command.configure(placeholder_text="Start curve")
    writehistory("> Curve\nStart curve")
    viewport.focus_set()
def startjoin(event=None):
    global activecommand
    activecommand = 'join'
    command.delete(0, 'end')
    command.configure(placeholder_text="Select objects to join")
    writehistory("> Join\nSelect objects to join")
    viewport.focus_set()
def startexplode(event=None):
    global activecommand
    activecommand = "explode"
    command.delete(0, "end")
    command.configure(placeholder_text = "Select objects to explode")
    writehistory("> Explode\nSelect objects to explode")
    viewport.focus_set()
def startrectangle(event=None):
    global activecommand
    activecommand = 'rectangle'
    command.delete(0, 'end')
    command.configure(placeholder_text = "Start rectangle")
    writehistory("> Rectangle\nStart rectangle")
    viewport.focus_set()
def starttext(event=None):
    global activecommand
    activecommand = 'text'
    command.delete(0, 'end')
    command.configure(placeholder_text='Choose text position')
    writehistory("> Text\nChoose text position")
    viewport.focus_set()
def startplaceholdercmd(name, message):
    global activecommand
    activecommand = name.lower().replace(" ", "")
    command.delete(0, "end")
    command.configure(placeholder_text=message)
    writehistory(F"> {name}\n{message}")
    viewport.focus_set()
def runnamedcommand(name):
    global lastcommand
    normalized = name.lower().replace(" ", "")
    actions = {
        "polyline": startpolyline,
        'pline': startpolyline,
        'curve': startcurve,
        'crv': startcurve,
        "join": startjoin,
        "explode": startexplode,
        'exp': startexplode,
        'rectangle': startrectangle,
        "rect": startrectangle,
        'text': starttext,
        'txt': starttext,
        'distance':lambda: startplaceholdercmd("Distance", "Select first point"),
        "angle": lambda: startplaceholdercmd("Angle", "Select first point"),
        "revolve": lambda: startplaceholdercmd("Revolve", "Select objects to revolve"),
        "extrude": lambda: startplaceholdercmd("Extrude", "Select objects to extrude"),
        "mirror": lambda: startplaceholdercmd("Mirror", "Select objects to mirror"),
        "copy": lambda: startplaceholdercmd( "Copy", "Select objects to copy"),
        "new": lambda: startplaceholdercmd("New", "New file is not implemented yet"),
        "save": lambda: startplaceholdercmd("Save", "Save is not implemented"),
        "3dm": lambda: startplaceholdercmd("3DM", "3DM export is not implemented yet"),
        "dxf": lambda: startplaceholdercmd("DXF", "DXF export is not implemented yet"),
        "file": open_file_menu,
        "analyze": openanalyzemenu,
        "tools": opentoolsmenu,
        'saveas': opensaveascommand,
        "png": savepngcommand,
        "circle": lambda: startplaceholdercmd("Circle", "Choose circle center"),
        "fillet": lambda: startplaceholdercmd("Fillet", "Select curves to fillet"),
        "trim": lambda: startplaceholdercmd("Trim", "Select objects to trim")
    }
    action = actions.get(normalized)
    if action is None:
        return False
    if normalized not in ("file", "analyze", "tools"):
        lastcommand = name
    action()
    return True
def repeatlastcommand():
    global lastcommand
    if lastcommand is None:
        return
    previous = lastcommand
    runnamedcommand(previous)
def cancelactivecommand(event=None):
    global activecommand
    if activecommand is None:
        return
    writehistory("Command ended")
    activecommand = None
    command.delete(0, 'end')
    command.configure(placeholder_text = "Command:")
    viewport.focus_set()
    return "break"
app.bind("<Escape>", cancelactivecommand)
command.bind("<Escape>", cancelactivecommand)
viewport.bind("<Escape>", cancelactivecommand)
def runcmd(event):
    global history_index
    typed = command.get().strip()
    if not typed:
        return 'break'
    command.delete(0, 'end')
    past_commands.append(typed)
    history_index = len(past_commands)
    if not runnamedcommand(typed):
        writehistory(f"> {typed}\nCommand not found")
        command.configure(placeholder_text = "Command:")
        viewport.focus_set()
    return 'break'
command.bind("<Return>", runcmd)
commandnames = ["Polyline", "Curve", "Join", "Explode", "Rectangle", "Text", "File", "New", "Save", "Save as", '3DM', "PNG", "DXF", "Analyze", "Distance", "Angle", "Tools", "Revolve", "Extrude", "Mirror", "Copy", 
                'Circle', "Fillet", "Trim"]

command._entry.configure(selectbackground="#666666", selectforeground="#f5E8D2")
def updatecommandsuggestion(event=None):
    if activecommand is not None:
        return
    if event is not None and event.keysym in ( "Return", "Up", "Down", "Left", "Right", "Escape", "Tab", "bracketright"):
        return
    typed = command.get()
    if not typed:
       return
    typedkey = typed.lower().replace(" ", "")
    match = next((name for name in commandnames if name.lower().replace(" ", "").startswith(typedkey) and 
                  name.lower().replace(" ", "") != typedkey), None)
    if match is None:
        return
    typedlength = len(typed)
    command.delete(0, "end")
    command.insert(0, match)
    command._entry.selection_range(typedlength, "end")
    command._entry.icursor(typedlength)
def commandbackspace(event):
    try:
        selectionstart = int(command._entry.index("sel.first"))
    except Exception:
        return
    typedpart = command.get()[:selectionstart]
    if typedpart:
        typedpart = typedpart[:-1]
    command.delete(0, 'end')
    command.insert(0, typedpart)
    command._entry.icursor('end')
    app.after_idle(updatecommandsuggestion)
    return 'break'
command.bind("<KeyRelease>", updatecommandsuggestion, add="+")
command.bind("<KeyPress-BackSpace>", commandbackspace, add="+")


def toggle2dshort(event=None):
    if renderer is not None:
        renderer.key_event("2", True)
        renderer.key_event("2", False)
    viewport.focus_set()
    return "break"
command.bind("<KeyPress-bracketright>", toggle2dshort)
app.bind("<KeyPress-bracketright>", toggle2dshort)
def typecmduni(event):
    global activecommand
    if event.state & 0x0004:
        return
    focused = app.focus_get()
    if focused is not None and focused.winfo_class() in ("Entry", "Text"):
        return
    if activecommand is not None:
        return
    if event.char and event.char.isprintable():
        command.focus_set()
        command.insert("end", event.char)
        app.after_idle(updatecommandsuggestion)
        return 'break'
app.bind("<KeyPress>", typecmduni, add="+")
def browse_commands(event):
    global history_index
    if not past_commands:
        return "break"
    if event.keysym == 'Up':
        history_index = max(0, history_index-1)
    else:
        history_index = min(len(past_commands), history_index+1)
    command.delete(0, "end")
    if history_index < len(past_commands):
        command.insert(0, past_commands[history_index])
    return "break"
command.bind("<Up>", browse_commands)
command.bind("<Down>", browse_commands)

def saveviewportpng():
    closefilemenu()
    app.after(250, openpngdialog)
def openpngdialog():
    app.update_idletasks()
    x, y = viewport.winfo_rootx(), viewport.winfo_rooty()
    w, h = viewport.winfo_width(), viewport.winfo_height()
    shot = ImageGrab.grab(bbox=(x, y, x+w, y+h), all_screens=True)
    vx, vy=x - app.winfo_rootx(), y - app.winfo_rooty()
    selected = [0, 0, w, h]
    dragstart = [None]
    dragbox = [None]
    selecting = [False]
    overlay = Toplevel(app)
    overlay.overrideredirect(True)
    overlay.geometry(f"{app.winfo_width()}x{app.winfo_height()}" f"+{app.winfo_rootx()}+{app.winfo_rooty()}")
    overlay.attributes("-alpha", 0.55)
    shade = Canvas(overlay, bg='black', highlightthickness=0, cursor='arrow')
    shade.pack(fill='both', expand=True)
    diaglog  = ctk.CTkToplevel(app)
    diaglog.overrideredirect(True)
    diaglog.configure(fg_color='#242B23')
    dx = app.winfo_rootx() + (app.winfo_width()-560)//2
    dy = app.winfo_rooty() + (app.winfo_height()-320)//2
    diaglog.geometry(f"560x320+{dx}+{dy}")
    panel = ctk.CTkFrame(diaglog, fg_color="#3B322A", corner_radius=8, border_color="#A66b3E", border_width=2)
    panel.pack(fill='both', expand=True, padx=4, pady=4)
    diaglog.lift()
    def follow_app(event):
        if event.widget is not app:
            return
        ax, ay = app.winfo_rootx(), app.winfo_rooty()
        aw, ah = app.winfo_width(), app.winfo_height()
        overlay.geometry(f"{aw}x{ah}+{ax}+{ay}")
        diaglog.geometry(f"560x320+{ax+(aw-560)//2}+{ay+(ah-320)//2}")
    follow_id = app.bind("<Configure>", follow_app, add="+")
    def close():
        app.unbind("<Configure>", follow_id)
        diaglog.destroy()
        overlay.destroy()
    def textbutton(text, width, height, command, fontsize=18):
        button=Canvas(panel, width=width, height=height, bg="#3B322A", highlightthickness=0, cursor='hand2')
        label = button.create_text(width//2, height//2, text=text, fill='#F5E8D2', font=("Iceland", fontsize))
        button.label_id = label
        button.enabled = True
        button.bind("<Enter>", lambda e: button.itemconfig(label, fill='#F0AA60' if button.enabled else "#777777"))
        button.bind("<Leave>", lambda e: button.itemconfig(label, fill='#F5E8D2' if button.enabled else "#777777"))
        button.bind("<Button-1>", lambda e: command() if button.enabled else None)
        return button
    def pixelbox():
        return (  round(selected[0] * shot.width / w),  round(selected[1] * shot.height / h),  round(selected[2] * shot.width / w),  round(selected[3] * shot.height / h))
    scale = StringVar(value="100")
    ctk.CTkLabel(panel, text="Save as PNG", font=("Lexend", 17)).place(x=22, y=15)
    textbutton("×", 40, 40, close, 24).place(x=495, y=8)
    ctk.CTkLabel(panel, text="Scale (%)", font=("Lexend", 12)).place(x=24, y=75)
    scale_entry = ctk.CTkEntry(panel, width=95, textvariable=scale)
    scale_entry.place(x=24, y=106)
    ctk.CTkLabel(panel, text="Preview", font=("Lexend", 12)).place(x=230, y=42)
    preview = ctk.CTkLabel(panel, text="", width=290, height=170, fg_color="#242B23" )
    preview.place(x=230, y=70)
    dimensions = ctk.CTkLabel(panel, text='', font=("Lexend", 11))
    dimensions.place(x=230, y=245)
    def refresh(*_):
        try:
            percent = int(scale.get())
        except ValueError:
            percent = 0
        valid = 1 <= percent <= 100
        scale_entry.configure(border_color = "#E28B45" if valid else "#B9533C" )
        save_button.enabled = valid
        save_button.itemconfig(save_button.label_id, fill="#F5E8D2" if valid else "#777777")
        save_button.configure(cursor="hand2" if valid else "arrow")
        if not valid:
            return
        cropped = shot.crop(pixelbox())
        out_w = max(1, round(cropped.width * percent/100))
        out_h = max(1, round(cropped.height * percent / 100))
        image = cropped.resize((out_w, out_h), Image.Resampling.LANCZOS)
        image.thumbnail((280, 160), Image.Resampling.LANCZOS)
        preview.configure(image=ctk.CTkImage(light_image=image, dark_image=image, size=image.size))
        dimensions.configure(text=f"{out_w} x {out_h} px")
    def set_area():
        selecting[0] = True
        diaglog.withdraw()
        overlay.attributes("-alpha", 0.25)
        shade.focus_set()
    def position(event):
        return(max(0, min(w, event.x-vx)), max(0, min(h, event.y-vy)))
    def insideview(event):
        return vx <= event.x <= vx+w and vy <= event.y <= vy+h
    def cancel_drag():
        if dragbox[0] is not None:
            shade.delete(dragbox[0])
            dragbox[0] = None
        dragstart[0] = None

    def star_drag(event):
        if not selecting[0]:
            return
        if not (vx <= event.x<=  vx +w and vy <= event.y <= vy+h):
            return
        dragstart[0] = position(event)
        dragbox[0] = shade.create_rectangle( event.x, event.y, event.x, event.y, outline="#F0AA60", width=3)
    def movedrag(event):
        if not selecting[0] or dragstart[0] is None:
            return
        if dragstart[0] is None:
                return
        px, py = position(event)
        sx, sy = dragstart[0]
        shade.coords(dragbox[0], vx+sx, vy+sy, vx+px, vy+py)
    def enddrag(event):
        if not selecting[0] or dragstart[0] is None:
            return
        if dragstart[0] is None:
            return
        sx, sy = dragstart[0]
        px, py = position(event)
        if abs(px-sx) >= 5 and abs(py-sy) >= 5:
            selected[:] = [min(sx, px), min(sy, py), max(sx, px), max(sy, py)]
        cancel_drag()
        selecting[0] = False
        shade.configure(cursor="arrow")
        overlay.attributes("-alpha", 0.55)
        diaglog.deiconify()
        diaglog.lift()
        refresh()
    def save():
        path = filedialog.asksaveasfilename(parent=diaglog, title='Save viewport as PNG', defaultextension=".png", filetypes=[("PNG image", "*.png")], initialfile = 'Armor3D.png')
        if not path:
            return
        cropped = shot.crop(pixelbox())
        percent = int(scale.get())
        size = (max(1, round(cropped.width * percent /100)), max(1, round(cropped.height * percent/100)))
        cropped.resize(size, Image.Resampling.LANCZOS).save(path, "PNG")
        close()
    textbutton("Set", 95, 42, set_area).place(x=18, y=164)
    save_button = textbutton("Save", 115, 42, save)
    save_button.place(x=412, y=258)
    def bringdialogfront():
        if diaglog.winfo_exists() and not selecting[0]:
            diaglog.lift()
            diaglog.focus_set()
    def shadeclick(event):
        if selecting[0]:
            star_drag(event)
        else:
            app.after_idle(bringdialogfront)
            return "break"
    shade.bind("<ButtonPress-1>", shadeclick)
    shade.bind("<ButtonRelease-1>", enddrag)
    shade.bind("<B1-Motion>", movedrag)
    shade.bind("<Escape>", lambda event: close())
    shade.bind("<Motion>", lambda e: shade.configure(cursor='crosshair' if selecting[0] and insideview(e) else "arrow"))
    shade.bind("<Leave>", lambda e: cancel_drag())
    diaglog.bind("<Escape>", lambda event: close())
    scale.trace_add("write", refresh)
    refresh()
        
filez = canvas.create_text(24, 8, text="File", font=("Lexend", 8), fill='#F5E8D2')
canvas.tag_bind(filez, "<Enter>", lambda event: canvas.itemconfig(filez, fill="#F0AA60"))
canvas.tag_bind(filez, "<Leave>", lambda event: canvas.itemconfig(filez, fill="#F5E8D2"))
filemenu = Canvas(app, width=160, height=104, bg="#3B322A",  highlightthickness=1, highlightbackground="#A66B3E")
menu_rows = []
for i, name in enumerate(("New", "Save", "Save As")): 
    y=4 + i * 32
    box = filemenu.create_rectangle(4, y, 155, y +30, fill="", outline="")
    label = filemenu.create_text(12, y+15, text=name, anchor='w', fill="#F5E8D2", font=("Iceland", 13))
    menu_rows.append((box, label))
def menu_motion(event):
    hovered = (event.y-4) // 32
    for i, (box, label) in enumerate(menu_rows):
        active = i == hovered and 4 <= event.x <= 155
        filemenu.itemconfig(box, fill='#67442F' if active else "")
        filemenu.itemconfig(label, fill= "#67442F" if active else "")
        filemenu.itemconfig(label, fill="#F0AA60" if active else "#F5E8D2")
filemenu.bind("<Motion>", menu_motion)
filepage = "main"
def showfilepage(page):
    global filepage
    filepage = page
    names = ("New", "Save", "Save As") if page == "main" else ("3DM", "PNG", "DXF")
    for (box, label), name in zip(menu_rows, names):
        filemenu.itemconfig(box, fill='')
        filemenu.itemconfig(label, text=name, fill="#F5E8D2")
def filemenuclick(event):
    if not (4 <= event.x <= 155):
        return
    row = (event.y -4)// 32
    if row not in (0, 1, 2):
        return
    if filepage =='main':
        names=  ("New", "Save", "Save As")
        if row == 2:
            showfilepage("formats")
        else:
            closefilemenu()
            runnamedcommand(names[row])
    else:
        names = ("3DM", "PNG", "DXF")
        closefilemenu()
        runnamedcommand(names[row])
filemenu.bind("<Button-1>", filemenuclick)
file_hover_job = None
def open_file_menu():
    global file_hover_job
    file_hover_job = None
    slidemenu(analyze_menu, 100, 72, False)
    slidemenu(toolsmenu, 150, 136, False)
    showfilepage("main")
    slidemenu(filemenu, 8, 104, True)
def cancelfilehover(event=None):
    global file_hover_job
    if file_hover_job is not None:
        app.after_cancel(file_hover_job)
        file_hover_job = None
def file_enter(event):
    global file_hover_job
    cancelfilehover()
    file_hover_job = app.after(500, open_file_menu)
def file_click(event):
    cancelfilehover()
    if filemenu.winfo_manager():
        slidemenu(filemenu, 8, 104, False)
    else:
        open_file_menu()
fileclosejob = None

def pointeronfilemenu(event):
    x=  event.x_root- app.winfo_rootx()
    y = event.y_root - app.winfo_rooty()
    return (8 <= x <= 45 and 0 <= y <= 20) or (8 <= x <= 170 and 20 <= y <= 158)
def cancelfileclose():
    global fileclosejob
    if fileclosejob is not None:
        app.after_cancel(fileclosejob)
        fileclosejob = None
def closefilemenu():
    global fileclosejob
    fileclosejob = None
    slidemenu(filemenu, 8, 104, False)
def filepointermtion(event):
    if filemenu in menusliding:
        return
    global fileclosejob
    if not filemenu.winfo_manager():
        return
    if pointeronfilemenu(event):
        cancelfileclose()
    elif fileclosejob is None:
        fileclosejob = app.after(180, closefilemenu)
def file_outside_click(event):
    if filemenu.winfo_manager() and not pointeronfilemenu(event):
        cancelfileclose()
        closefilemenu()
app.bind_all("<Motion>", filepointermtion, add="+")
app.bind_all("<Button-1>", file_outside_click, add="+")
canvas.tag_bind(filez, "<Enter>", file_enter, add="+")
canvas.tag_bind(filez, "<Leave>", cancelfilehover, add="+")
canvas.tag_bind(filez, "<Button-1>", file_click)



importz = canvas.create_text(68, 8, text='Import', font=("Lexend", 8), fill='#F5E8D2')
canvas.tag_bind(importz, "<Enter>", lambda event: canvas.itemconfig(importz, fill='#F0AA60'))
canvas.tag_bind(importz, "<Leave>", lambda event: canvas.itemconfig(importz, fill='#F5E8D2'))

analyze = canvas.create_text(120, 8, text='Analyze', font=("Lexend", 8), fill='#F5E8D2')
analyze_menu = Canvas(app, width=160, height=72, bg="#3B332A", highlightthickness=1, highlightbackground="#A66B3E")
analyze_rows = []
for i, name in enumerate(("Distance", "Angle")):
    y = 4 + i *32
    box = analyze_menu.create_rectangle(4, y, 155, y +30, fill='', outline='')
    label = analyze_menu.create_text(12, y+15, text=name, anchor='w', fill='#F5E8D2', font=("Iceland", 12))
    analyze_rows.append((box, label))
def analyzemenumotion(event):
    hovered = (event.y-4) //32
    for i, (box, label) in enumerate(analyze_rows):
        active = i == hovered and 4 <= event.x <= 155
        analyze_menu.itemconfig(box, fill='#67442F' if active else "")
        analyze_menu.itemconfig(label, fill='#F0AA60' if active else "#F5E8D2")
analyze_menu.bind("<Motion>", analyzemenumotion)
def analyzemenuclick(event):
    if not (4<= event.x<= 155):
        return
    row = (event.y-4)//32
    names = ("Distance", "Angle")
    if 0<= row< len(names):
        closeanalyzemenu()
        runnamedcommand(names[row])
analyze_menu.bind("<Button-1>", analyzemenuclick)
analyzehoverjob = None
analyzeclosejob = None
def openanalyzemenu():
    global analyzehoverjob
    analyzehoverjob = None
    slidemenu(filemenu, 8, 104, False)
    slidemenu(toolsmenu, 150, 136, False)
    slidemenu(analyze_menu, 100, 72, True)
def analyze_enter(event):
    global analyzehoverjob
    canvas.itemconfig(analyze, fill="#F0AA60")
    if analyzehoverjob is not None:
        app.after_cancel(analyzehoverjob)
    analyzehoverjob = app.after(500, openanalyzemenu)
def analyzeleave(event):
    global analyzehoverjob
    canvas.itemconfig(analyze, fill="#F5E8D2")
    if analyzehoverjob is not None:
        app.after_cancel(analyzehoverjob)
        analyzehoverjob = None
def analyzeclick(event):
    global analyzehoverjob
    if analyzehoverjob is not None:
        app.after_cancel(analyzehoverjob)
        analyzehoverjob= None
    if analyze_menu.winfo_manager():
        slidemenu(analyze_menu, 100, 72, False)
    else:
        openanalyzemenu()
canvas.tag_bind(analyze, "<Button-1>", analyzeclick)
def closeanalyzemenu():
    global analyzeclosejob
    analyzeclosejob = None
    slidemenu(analyze_menu, 100, 72, False)
def analyzepointermotion(event):
    if analyze_menu in menusliding:
        return
    global analyzeclosejob
    if not analyze_menu.winfo_manager():
        return
    x = event.x_root - app.winfo_rootx()
    y= event.y_root - app.winfo_rooty()
    nearby = (94 <= x <= 148 and 0 <= y <= 20) or (100 <= x <= 260 and 20 <= y <= 94)
    if nearby:
        if analyzeclosejob is not None:
            app.after_cancel(analyzeclosejob)
            analyzeclosejob = None
    elif analyzeclosejob is None:
        analyzeclosejob = app.after(180, closeanalyzemenu)
canvas.tag_bind(analyze, "<Enter>", analyze_enter, add="+")
canvas.tag_bind(analyze, "<Leave>", analyzeleave, add="+")
app.bind_all("<Motion>", analyzepointermotion, add="+")

tools = canvas.create_text(170, 8, text="Tools", font=("Lexend", 8), fill="#F5E8D2")
toolsmenu = Canvas(app, width=160, height=136, bg="#3B322a", highlightthickness=1, highlightbackground="#A66B3E")
toolsrows = []
for i, name in enumerate(("Revolve", "Extrude", "Mirror", "Copy")):
    y = 4+i *32
    box = toolsmenu.create_rectangle(4, y, 155, y +30, fill='', outline='')
    label = toolsmenu.create_text(12, y+15, text=name, anchor='w', fill='#F5E8D2', font=("Iceland", 13))
    toolsrows.append((box, label))
def toolsmotion(event):
    hovered =(event.y - 4)// 32
    for i, (box, label) in enumerate(toolsrows):
        active = i == hovered and 4 <= event.x <=155
        toolsmenu.itemconfig(box, fill='#67442F' if active else "")
        toolsmenu.itemconfig(label, fill='#F0AA60' if active else "#F5E8D2")
toolsmenu.bind("<Motion>", toolsmotion)
def toolsmenuclick(event):
    if not (4<= event.x <= 155):
        return
    row = (event.y-4)//32
    names = ("Revolve", "Extrude", "Mirror", "Copy")
    if 0<= row < len(names):
        closetoolsmenu()
        runnamedcommand(names[row])
toolsmenu.bind("<Button-1>", toolsmenuclick)
toolshoverjob = None
toolsclosejob = None
def opentoolsmenu():
    global toolshoverjob
    toolshoverjob = None
    slidemenu(filemenu, 8, 104, False)
    slidemenu(analyze_menu, 100, 72, False)
    slidemenu(toolsmenu, 150, 136, True)
def closetoolsmenu():
    global toolsclosejob
    toolsclosejob = None
    slidemenu(toolsmenu, 150, 136, False)
def toolsenter(event):
    global toolshoverjob
    canvas.itemconfig(tools, fill='#F0AA60')
    if toolshoverjob is not None:
        app.after_cancel(toolshoverjob)
    toolshoverjob = app.after(500, opentoolsmenu)
def toolsleave(event):
    global toolshoverjob
    canvas.itemconfig(tools, fill='#F5E8D2')
    if toolshoverjob is not None:
        app.after_cancel(toolshoverjob)
        toolshoverjob = None
def toolsclick(event):
    toolsleave(event)
    if toolsmenu.winfo_manager():
        closetoolsmenu()
    else:
        opentoolsmenu()
def pointerontoolsmenu(event):
    x = event.x_root - app.winfo_rootx()
    y = event.y_root - app.winfo_rooty()
    return (150 <= x <= 190 and 0 <= y <= 20) or (150 <= x <= 310 and 20 <= y <= 156)
def toolspointermotion(event):
    global toolsclosejob
    if toolsmenu in menusliding or not toolsmenu.winfo_manager():
        return
    if pointerontoolsmenu(event):
        if toolsclosejob is not None:
            app.after_cancel(toolsclosejob)
            toolsclosejob = None
    elif toolsclosejob is None:
        toolsclosejob = app.after(180, closetoolsmenu)
def toolsoutsideclick(event):
    if toolsmenu.winfo_manager() and not pointerontoolsmenu(event):
        closetoolsmenu()
canvas.tag_bind(tools, "<Enter>", toolsenter)
canvas.tag_bind(tools, "<Leave>", toolsleave)
canvas.tag_bind(tools, "<Button-1>", toolsclick)
app.bind_all("<Motion>", toolspointermotion, add="+")
app.bind_all("<Button-1>", toolsoutsideclick, add="+")

AI  = canvas.create_text(226, 8, text='AI Creation', font=("Lexend", 8), fill='#F5E8D2')
canvas.tag_bind(AI, "<Enter>", lambda event: canvas.itemconfig(AI, fill='#F0AA60'))
canvas.tag_bind(AI, "<Leave>", lambda event: canvas.itemconfig(AI, fill='#F5E8D2'))


sidebar = ctk.CTkFrame(app, width=520, corner_radius=16, fg_color="#3B322A", border_color="#A66B3E", border_width=4)
sidebar.pack_propagate(False)
prompt_label = ctk.CTkLabel(sidebar, text="", font=("Iceland", 35), width=300, height=82, justify='center')
prompt_label.pack(pady=(24, 10))
ai_input = ctk.CTkEntry( sidebar, placeholder_text="Start typing...", font=("Lexend", 12), fg_color="#3B322A", border_color="#E28B45",  text_color="#F5E8D2", placeholder_text_color="#C5B29A")
ai_input.place(relx=0.5, rely=1, y=-15, anchor="s", relwidth=0.8)
ai_input.configure(height=38)
typingjob = None
def start_typing():
    global typingjob
    message = "What's on your\nmind today?"
    if typingjob is not None:
        app.after_cancel(typingjob)
    def type_next(count):
        global typingjob
        prompt_label.configure(text=message[:count])
        if count < len(message):
            typingjob = app.after(55, lambda: type_next(count+1))
        else:
            typingjob = None
    type_next(0)
def toggleai(event=None):
    global animating, current_offset
    if animating:
        return
    opening = not sidebar.winfo_manager()
    offscreen = app.winfo_width() * panel_ratio + 16
    start, end = (offscreen, 16) if opening else (16, offscreen)
    animating = True
    if opening:
        sidebar.place(relx=1, x=start, y=0, anchor="ne",
                      relwidth=0.5, relheight=1)
        resize_cmd_boxes(canvas.winfo_width())
        for line in shadow_lines:
            canvas.itemconfigure(line, state="normal")
    def animate(step):
        global animating, current_offset
        progress = 1 - (1 - step / 12) ** 3
        current_offset = start + (end - start) * progress
        sidebar.place(relx=1, x=current_offset, y=0, anchor="ne",
                      relwidth=0.5, relheight=1)
        update_shadow(app.winfo_width(), app.winfo_height(), current_offset)
        resize_cmd_boxes(canvas.winfo_width())
        if step < 12:
            app.after(16, lambda: animate(step + 1))
        else:
            animating = False
            if opening:
                start_typing()
            else:
                sidebar.place_forget()
                for line in shadow_lines:
                    canvas.itemconfigure(line, state="hidden")
                resize_cmd_boxes(canvas.winfo_width())
    animate(0)
close_canvas = Canvas(sidebar, width=32, height=32, bg="#3B322A", highlightthickness=0)
close_canvas.place(x=12, y=12)
close_x = close_canvas.create_text(16, 16, text="×", fill='white', font=("Lexend", 20))
close_canvas.tag_bind(close_x, "<Button-1>", toggleai)
canvas.tag_bind(AI, "<Button-1>", toggleai)
resize_handle = Canvas(sidebar, width=12, bg="#3B322A", highlightthickness=0, cursor="sb_h_double_arrow")
resize_handle.place(x=0, y=18, relheight=1, height=-36)
def drag_sidebar(event):
    global panel_ratio
    if animating:
        return
    window_width = canvas.winfo_width()
    rightedge = app.winfo_rootx() + window_width + 16
    new_width = rightedge-event.x_root
    new_width = max(320, min(window_width-80, new_width))
    panel_ratio = new_width / window_width
    sidebar.place(relx=1, x=16, y=0, anchor='ne', relwidth=panel_ratio, relheight=1)
    update_shadow(window_width, canvas.winfo_height(), 16)
    resize_cmd_boxes(window_width)
resize_handle.bind("<B1-Motion>", drag_sidebar)

icon = Image.open(getpath("Assets/polylinez.png")).convert("RGBA")
bounds = icon.getbbox()
if bounds:
    icon = icon.crop(bounds)
icon.thumbnail((30, 30), Image.Resampling.LANCZOS)
polylinenormal = ImageTk.PhotoImage(icon)
polyline_hover = ImageTk.PhotoImage(ImageEnhance.Brightness(icon).enhance(0.6))
polyline_square = canvas.create_rectangle(8, 103, 52, 147, fill='', outline='')
polyline_icon = canvas.create_image(30, 125, image=polylinenormal)
canvas.tag_bind(polyline_square, "<Button-1>", lambda event: runnamedcommand("Polyline"))
canvas.tag_bind( polyline_icon, "<Button-1>",lambda event: runnamedcommand("Polyline"))
def polyline_motion(event):
    hovering = 8 <= event.x<= 52 and 103 <= event.y <= 147
    canvas.itemconfig(polyline_square, fill="#67442F"if hovering else  "", outline="#E28B45" if hovering else "")
    canvas.itemconfig(polyline_icon, image=polyline_hover if hovering else polylinenormal)


curve_image = Image.open(getpath("Assets/curvez.png")).convert("RGBA")
bounds = curve_image.getbbox()
if bounds:
    curve_image = curve_image.crop(bounds)
curve_image.thumbnail((30, 30), Image.Resampling.LANCZOS)
curve_normal = ImageTk.PhotoImage(curve_image)
curve_hover = ImageTk.PhotoImage(ImageEnhance.Brightness(curve_image).enhance(0.6))
curve_square = canvas.create_rectangle(53, 103, 96, 147, fill='', outline='')
curve_icon = canvas.create_image(75, 125, image=curve_normal)
canvas.tag_bind( curve_square, "<Button-1>", lambda event: runnamedcommand("Curve"))
canvas.tag_bind( curve_icon, "<Button-1>", lambda event: runnamedcommand("Curve"))
def curve_motion(event):
    hovering = 53 <= event.x <=97 and 103 <= event.y <=147
    canvas.itemconfig(curve_square, fill='#67442F' if hovering else '', outline="#E28B45" if hovering else "")
    canvas.itemconfig(curve_icon, image=curve_hover if hovering else curve_normal)


puzzle_image = Image.open(getpath("Assets/joinz.png")).convert("RGBA")
bounds = puzzle_image.getbbox()
if bounds:
    puzzle_image = puzzle_image.crop(bounds)
puzzle_image.thumbnail((30, 30), Image.Resampling.LANCZOS)
puzzle_normal = ImageTk.PhotoImage(puzzle_image)
puzzle_hover= ImageTk.PhotoImage(ImageEnhance.Brightness(puzzle_image).enhance(0.6))
puzzle_square  = canvas.create_rectangle(8, 148, 52, 192, fill='', outline='')
puzzle_icon = canvas.create_image(30, 170, image=puzzle_normal)
canvas.tag_bind(puzzle_square, "<Button-1>", lambda event: runnamedcommand("Join"))
canvas.tag_bind(puzzle_icon, "<Button-1>", lambda event: runnamedcommand("Join"))
def puzzlemotion(event):
    hovering = 8 <= event.x <= 52 and 148 <= event.y <= 192
    canvas.itemconfig(puzzle_square, fill='#67442F' if hovering else "", outline="#E28B45" if hovering else "")
    canvas.itemconfig(puzzle_icon, image=puzzle_hover if hovering else puzzle_normal)


explode_image = Image.open(getpath("Assets/explode.png")).convert("RGBA")
bounds = explode_image.getbbox()
if bounds: 
    explode_image = explode_image.crop(bounds)
explode_image.thumbnail((30, 30), Image.Resampling.LANCZOS)
explode_normal = ImageTk.PhotoImage(explode_image)
explode_hover = ImageTk.PhotoImage(ImageEnhance.Brightness(explode_image).enhance(0.6))
explode_square = canvas.create_rectangle(53, 148, 97, 192, fill='', outline='')
explode_icon = canvas.create_image(75, 170, image=explode_normal)
canvas.tag_bind(explode_square, "<Button-1>", lambda event: runnamedcommand("Explode"))
canvas.tag_bind(explode_icon, "<Button-1>", lambda event: runnamedcommand("Explode"))
def explode_motion(event):
    hovering = 53 <= event.x <= 97 and 148 <= event.y <=192
    canvas.itemconfig(explode_square, fill='#67442F' if hovering else "", outline="#E28B45" if hovering else "")
    canvas.itemconfig(explode_icon, image=explode_hover if hovering else explode_normal)


rectangle_image = Image.open(getpath("Assets/rectangle.png")).convert("RGBA")
bounds = rectangle_image.getbbox()
if bounds:
    rectangle_image = rectangle_image.crop(bounds)
rectangle_image.thumbnail((30, 30), Image.Resampling.LANCZOS)
rectangle_normal = ImageTk.PhotoImage(rectangle_image)
rectangle_hover = ImageTk.PhotoImage(ImageEnhance.Brightness(rectangle_image).enhance(0.6))
rectangle_sqaure = canvas.create_rectangle(8, 193, 52, 237, fill='', outline='')
rectangle_icon = canvas.create_image(30, 215, image=rectangle_normal)
canvas.tag_bind(rectangle_sqaure, "<Button-1>", lambda event: runnamedcommand("Rectangle"))
canvas.tag_bind(rectangle_icon, "<Button-1>", lambda event: runnamedcommand("Rectangle"))
def rectangle_motion(event):
    hovering = 8 <= event.x <= 52 and 193 <= event.y <= 237
    canvas.itemconfig(rectangle_sqaure, fill="#67442F" if hovering else "",  outline="#E28B45" if hovering else "")
    canvas.itemconfig(rectangle_icon,  image=rectangle_hover if hovering else rectangle_normal)


text_image = Image.open(getpath("Assets/text.png")).convert("RGBA")
bounds = text_image.getbbox()
if bounds:
    text_image = text_image.crop(bounds)
text_image.thumbnail((30, 30), Image.Resampling.LANCZOS)
text_normal = ImageTk.PhotoImage(text_image)
text_hover = ImageTk.PhotoImage(ImageEnhance.Brightness(text_image).enhance(0.6))
text_square = canvas.create_rectangle(53, 193, 97, 237, fill="", outline="")
text_icon = canvas.create_image(75, 215, image=text_normal)
canvas.tag_bind(text_square, "<Button-1>", lambda event: runnamedcommand("Text"))
canvas.tag_bind(text_icon, "<Button-1>", lambda event: runnamedcommand("Text"))
extrabox = canvas.create_rectangle(8, 248, 92, 284, fill='#242b23', outline='', width=2)
extratext = canvas.create_text(50, 266, text="Extra", font=("Iceland", 14), fill="#F5E8D2")
def extraenter(event):
    canvas.itemconfig(extrabox, fill="#67442f", outline="#E28B45")
    canvas.itemconfig(extratext, fill='#F0AA60')
def extraleave(event):
    canvas.itemconfig(extrabox, fill="#242b23", outline="")
    canvas.itemconfig(extratext, fill='#F5E8D2')
for item in (extrabox, extratext):
    canvas.tag_bind(item, "<Enter>", extraenter)
    canvas.tag_bind(item, "<Leave>", extraleave)

def bindtoolhover(left, top, right, bottom, box, icon, normal, hover):
    canvas.itemconfig(box, fill="#242B23", outline="")
    def update():
        mouse_x = canvas.winfo_pointerx() - canvas.winfo_rootx()
        mouse_y = canvas.winfo_pointery() - canvas.winfo_rooty()
        hovering = left <= mouse_x <= right and top <= mouse_y <= bottom
        canvas.itemconfig(box, fill='#67442F' if hovering else "#242B23", outline="#E28B45" if hovering else "")
        canvas.itemconfig(icon, image=hover if hovering else normal)
    def enter(event):
        canvas.itemconfig(box, fill="#67442F", outline="#E28B45")
        canvas.itemconfig(icon, image=hover)
        canvas.tag_raise(icon)
    def leave(event):
        app.after(10, update)
    for item in (box, icon):
        canvas.tag_bind(item, "<Enter>", enter)
        canvas.tag_bind(item, "<Leave>", leave)
bindtoolhover(8, 103, 52, 147, polyline_square, polyline_icon, polylinenormal, polyline_hover)
bindtoolhover(53, 103, 97, 147, curve_square, curve_icon, curve_normal, curve_hover)
bindtoolhover(8, 148, 52, 192, puzzle_square, puzzle_icon, puzzle_normal, puzzle_hover)
bindtoolhover(53, 148, 97, 192, explode_square, explode_icon, explode_normal, explode_hover)
bindtoolhover(8, 193, 52, 237, rectangle_sqaure, rectangle_icon, rectangle_normal, rectangle_hover)
bindtoolhover(53, 193, 97, 237, text_square, text_icon, text_normal, text_hover)

toolbarbuttons = [(8, 103, 52, 147, polyline_square, polyline_icon,
     polylinenormal, polyline_hover), (53, 103, 97, 147, curve_square, curve_icon,
     curve_normal, curve_hover), (8, 148, 52, 192, puzzle_square, puzzle_icon,
     puzzle_normal, puzzle_hover),
    (53, 148, 97, 192, explode_square, explode_icon,
explode_normal, explode_hover), (8, 193, 52, 237, rectangle_sqaure, rectangle_icon, rectangle_normal, rectangle_hover), (53, 193, 97, 237, text_square, text_icon,text_normal, text_hover),]
def toolbarhover(event):
    overbutton = False
    for left, top, right, bottom, box, icon, normal, hover in toolbarbuttons:
        hovering = left <= event.x <= right and top <= event.y <= bottom
        canvas.itemconfig(box, fill='#67442f' if hovering else "#242B23", outline="#E28B45" if hovering else "")
        canvas.itemconfig(icon, image=hover if hovering else normal)
        if hovering:
            canvas.tag_raise(icon)
            overbutton = True
    canvas.configure(cursor='hand2' if overbutton else "")
canvas.bind("<Motion>", toolbarhover, add="+")
def text_motion(event):
    hovering = 53 <= event.x <= 97 and 193 <= event.y <= 237
    canvas.itemconfig(text_square, fill="#67442F" if hovering else "", outline="#E28B45" if hovering else "")
    canvas.itemconfig(text_icon, image=text_hover if hovering else text_normal)


tooltips = [(8, 103, 52, 147, "Polyline"), (53, 103, 97, 147, "Curve"), (8, 148, 52, 192, "Join"), (53, 148, 97, 192, "Explode"), (8, 193, 52, 237, "Rectangle"), (53, 193, 97, 237, "Text")]

tooltip_job = None
tooltiptarget = None
tooltipwindow = None
def showtooltip(name, top):
    global tooltip_job, tooltipwindow
    tooltip_job = None
    tooltipwindow = ctk.CTkToplevel(app)
    tooltipwindow.overrideredirect(True)
    tooltipwindow.attributes("-topmost", True)
    tooltipwindow.configure(fg_color = "#34291f")
    label = ctk.CTkLabel(tooltipwindow, text=name, font=("Iceland", 14), text_color="#F5E8D2", fg_color="#34291f", corner_radius=5)
    label.pack(ipadx=9, ipady=5)
    tooltipwindow.update_idletasks()
    x = canvas.winfo_rootx() + 108
    y = canvas.winfo_rooty() + top +10
    tooltipwindow.geometry(f'+{x}+{y}')
    tooltipwindow.lift()
def hidetooltip(event=None):
    global tooltip_job, tooltiptarget, tooltipwindow
    if tooltip_job is not None:
        app.after_cancel(tooltip_job)
        tooltip_job = None
    if tooltipwindow is not None:
        tooltipwindow.destroy()
        tooltipwindow = None
    tooltiptarget = None
def tooltipmotion(event):
    global tooltip_job, tooltiptarget
    target = next(((name, top) for left, top, right, bottom, name in tooltips if left <= event.x <= right and top <= event.y <=bottom), None)
    if target == tooltiptarget:
        return
    hidetooltip()
    tooltiptarget = target
    if target is not None:
        tooltip_job = app.after(650, lambda current=target: showtooltip(*current))
canvas.bind("<Motion>", tooltipmotion, add="+")
canvas.bind("<Leave>", hidetooltip, add="+")

gridsnaptext = canvas.create_text(50, 315, text="Grid Snap", font=("Iceland", 13), fill='#F5E8D2', anchor='center')
gridsnapon = False
def showgridsnap(hovering=False):
    color = "#F0AA60" if gridsnapon else "#E28B45" if hovering else "#f5e8d2"
    canvas.itemconfig(gridsnaptext, fill=color)
def gridsnapmotion(event):
    hovering = 8 <= event.x <= 92 and 297 <= event.y <= 333
    showgridsnap(hovering)
def gridsnapclick(event):
    global gridsnapon
    if 8 <= event.x <= 92 and 297 <= event.y <= 333:
        gridsnapon = not gridsnapon
        showgridsnap(True)
canvas.bind("<Motion>", gridsnapmotion, add="+")
canvas.bind("<Button-1>", gridsnapclick, add="+")
canvas.bind("<Leave>", lambda event: showgridsnap(False), add="+")

orthotext = canvas.create_text(50, 355, text='Ortho', font=("Iceland", 13), fill='#F5E8D2', anchor='center')
orthoon = False
def showortho(hovering=False):
    color = "#f0aa60" if orthoon else "#e28b45" if hovering else "#F5E8d2"
    canvas.itemconfig(orthotext, fill=color)
def orthomotion(event):
    showortho(8 <= event.x <= 92 and 337 <= event.y <= 373)
def orthoclick(event):
    global orthoon
    if 8 <= event.x <= 92 and 337 <= event.y <= 373:
        orthoon = not orthoon
        showortho(True)
canvas.bind("<Motion>", orthomotion, add="+")
canvas.bind("<Button-1>", orthoclick, add="+")
canvas.bind("<Leave>", lambda event: showortho(False), add="+")

osnaptext = canvas.create_text(50, 395, text='Osnap', font=("Iceland", 13), fill='#F5E8d2', anchor='center')
onsapon = False
def showosnap(hovering=False):
    color = "#F0AA60" if onsapon else "#e28b45" if hovering else "#F5E8D2"
    canvas.itemconfig(osnaptext, fill=color)
def onsapmotion(event):
    showosnap(8 <= 8 <= event.x <= 92 and 377 <= event.y <= 413)
def osnapclick(event):
    global onsapon
    if 8 <= event.x <= 92 and 377 <= event.y <= 413:
        onsapon = not onsapon
        showosnap(True)
        refreshosnap()
canvas.bind("<Motion>", onsapmotion, add="+")
canvas.bind("<Button-1>", osnapclick, add="+")
canvas.bind("<Leave>", lambda event: showosnap(False), add="+")

canvas.create_line(0, 292, 100, 292, fill="#70543B", width=2)
canvas.create_line(0, 422, 100, 422, fill="#70543B", width=2)
canvas.create_line(0, 510, 100, 510, fill="#70543B", width=2)
canvas.create_text(50, 445, text="Layers", font=("Iceland", 15), fill="#F5E8D2", anchor="center")
layername = canvas.create_text(60, 485, text="Default", font=("Iceland", 11), fill="#F5E8D2", anchor="center")

layer_color = "#000000"
layer_swatch = canvas.create_rectangle(17, 477, 33, 493, fill=layer_color, outline="#70543B", width=1)
def swatch_enter(event):
    canvas.itemconfig(layer_swatch, outline="#E28B45", width=2)
def swatch_leave(event):
    canvas.itemconfig(layer_swatch, outline="#70543b", width=1)
colors = (("Black", "#000000"), ("Orange", "#E28B45"), ("Green", "#657b4f"), ("Brown", '#8A5837'))
layermenu = Canvas(app, width=152, height=128, bg="#3b322a", highlightthickness=1, highlightbackground="#A66B3e")
layer_rows = []
for i, (name, color) in enumerate(colors):
    y = 4+i *30
    background = layermenu.create_rectangle(4, y, 148, y+28, fill='', outline='')
    layermenu.create_rectangle(12, y+7, 26, y+21, fill=color, outline="#F5E8D2")
    label = layermenu.create_text(36, y+14, text=name, anchor='w', fill='#F5E8D2', font=("Iceland", 12))
    layer_rows.append((background, label))
def layermenumotion(event):
    hovered = (event.y-4)//30
    for i, (background, label) in enumerate(layer_rows):
        active = i == hovered and 4 <= event.x <=148
        layermenu.itemconfig(background, fill="#67442f" if active else "")
        layermenu.itemconfig(label, fill='#F0AA60' if active else "#F5E8D2")
def chooselayercolor(event):
    global layer_color
    index = (event.y-4) // 30
    if 0 <= index < len(colors):
        layer_color  =colors[index][1]
        canvas.itemconfig(layer_swatch, fill=layer_color)
        canvas.itemconfig(layername, text=colors[index][0])
        layermenu.place_forget()
def toggle_layer_menu(event):
    if layermenu.winfo_manager():
        layermenu.place_forget()
    else:
        menu_y = 497 if app.winfo_height() >= 635 else 298
        layermenu.place(x=8, y=menu_y)
def close_layer_outside(event):
    if event.widget == layermenu:
        return
    if event.widget == canvas and 17 <= event.x <= 33 and 477 <= event.y <= 493:
        return
    layermenu.place_forget()
canvas.tag_bind(layer_swatch, "<Enter>", swatch_enter)
canvas.tag_bind(layer_swatch, "<Leave>", swatch_leave)
canvas.tag_bind(layer_swatch, "<Button-1>", toggle_layer_menu)
layermenu.bind("<Motion>", layermenumotion)
layermenu.bind("<Button-1>", chooselayercolor)
app.bind_all("<Button-1>", close_layer_outside, add="+")

snapcanvas = Canvas(canvas, bg="#242B23", highlightthickness=0)
snapwindow = canvas.create_window( 0, 515, window=snapcanvas,  anchor="nw", width=100, height=230)
snapheading = snapcanvas.create_text(50, 15, text='Osnap', fill='#F5E8D2', font=("Iceland", 14))
snapenabled = {}
snapitems = {}
def togglesnap(name):
    global onsapon
    if name == 'Disable':
        onsapon= not onsapon
        showosnap()
    elif onsapon:
        snapenabled[name] = not snapenabled[name]
    refreshosnap()
def refreshosnap():
    active = onsapon
    snapcanvas.itemconfig(snapheading, fill='#F5E8D2' if active else "#777777")
    for name, (box, mark, label) in snapitems.items():
        if name == 'Disable':
            snapcanvas.itemconfig(mark, state='hidden' if active else 'normal')
            continue
        snapcanvas.itemconfig(box, outline="#E28B45" if active else "#555555")
        snapcanvas.itemconfig(label, fill="#F5E8D2" if active else "#777777")
        snapcanvas.itemconfig(mark, fill="#F0AA60" if active else "#777777")
        snapcanvas.itemconfig(mark, state="normal" if snapenabled[name] else "hidden")
for i, name in enumerate(("End", "Near", "Int", "Mid", "Cen", "Disable")):
    y = 48 + i *30
    snapenabled[name] = False
    box = snapcanvas.create_rectangle(18, y-7, 32, y+7, fill='#3B322A', outline="#E28B45", width=2)
    mark = snapcanvas.create_text(25, y, text="✓", fill='#F0AA60',font=("Iceland", 13), state='hidden')
    label = snapcanvas.create_text(60, y, text=name, fill="#F5E8D2", font=("Iceland", 13))
    snapitems[name] = (box, mark, label)
    for item in (box, mark, label):
        snapcanvas.tag_bind(item, "<Button-1>", lambda event, option=name: togglesnap(option))
snapcanvas.configure(scrollregion=(0, 0, 100, 225))
snapcanvas.bind("<MouseWheel>", lambda event: snapcanvas.yview_scroll(-1 if event.delta > 0 else 1, "units"))
refreshosnap()

menujobs = {}
menusliding = set()
def slidemenu(menu, x, height, opening):
    oldjob = menujobs.pop(menu, None)
    if oldjob is not None:
        app.after_cancel(oldjob)
    if not opening and not menu.winfo_manager():
        return
    start = menu.winfo_y() if menu.winfo_manager() else -height
    end = 20 if opening else -height
    menusliding.add(menu)
    def step(number):
        progress = 1 - (1-number /12) ** 3
        y = round(start + (end-start) * progress)
        menu.place(x=x, y=y)
        if number < 12: 
            menujobs[menu]  = app.after(16, lambda: step(number+1))
        else:
            menujobs.pop(menu, None)
            menusliding.discard(menu)
            if not opening:
                menu.place_forget()
    step(0)

def opensaveascommand():
    global activecommand
    activecommand = None
    command.delete(0, 'end')
    command.configure(placeholder_text = "Command: ")
    writehistory("> Save As\nChoose a format")
    open_file_menu()
    showfilepage("formats")
def savepngcommand():
    global activecommand
    activecommand = None
    command.delete(0, 'end')
    command.configure(placeholder_text="Command: ")
    writehistory("> PNG\nOpening PNG export")
    saveviewportpng()

extraoverlay = Toplevel(app)
extraoverlay.withdraw()
extraoverlay.overrideredirect(True)
extraoverlay.configure(bg="black")
extraoverlay.attributes("-alpha", 0.55)
extraoverlay.transient(app)
extradialog = ctk.CTkToplevel(app)
extradialog.withdraw()
extradialog.overrideredirect(True)
extradialog.configure(fg_color="#3B322A")
extradialog.transient(app)
extrapanel = ctk.CTkFrame(extradialog, fg_color="#3B322A",  border_color="#A66B3E", border_width=2, corner_radius=8)
extrapanel.pack(fill="both", expand=True, padx=3, pady=3)
extratitle = ctk.CTkLabel(extrapanel, text="Extra Tools", font=("Iceland", 24), text_color="#F5E8D2")
extratitle.place(x=28, y=20)
def positionextramenu(event=None):
    if event is not None and event.widget is not app:
        return
    app.update_idletasks()
    app_x = app.winfo_rootx()
    app_y = app.winfo_rooty()
    app_width = app.winfo_width()
    app_height = app.winfo_height()
    panel_width = min(720, app_width - 80)
    panel_height = min(360, app_height - 80)
    panel_x = app_x + (app_width - panel_width) // 2
    panel_y = app_y + (app_height - panel_height) // 2
    extraoverlay.geometry(f"{app_width}x{app_height}+{app_x}+{app_y}")
    extradialog.geometry( f"{panel_width}x{panel_height}+{panel_x}+{panel_y}")
def closeextramenu(event=None):
    extradialog.withdraw()
    extraoverlay.withdraw()
def extratoolbutton(name, x, y):
    button = Canvas( extrapanel,  width=112, height=44, bg="#3B322A",  highlightthickness=0, cursor="hand2")
    background = button.create_rectangle(2, 2, 110, 42, fill="#242B23", outline="#67442F", width=2)
    label = button.create_text(56, 22, text=name, fill="#F5E8D2", font=("Iceland", 15))
    def enter(event):
        button.itemconfig(background, fill="#67442F", outline="#E28B45")
        button.itemconfig(label, fill="#F0AA60")
    def leave(event):
        button.itemconfig( background, fill="#242B23", outline="#67442F")
        button.itemconfig(label, fill="#F5E8D2")
    def clicked(event):
        closeextramenu()
        runnamedcommand(name)
    button.bind("<Enter>", enter)
    button.bind("<Leave>", leave)
    button.bind("<Button-1>", clicked)
    button.place(x=x, y=y)
extratoolbutton("Circle", 30, 85)
extratoolbutton("Fillet", 160, 85)
extratoolbutton("Trim", 290, 85)
closeextra = Canvas(extrapanel,  width=38, height=38,  bg="#3B322A", highlightthickness=0, cursor="hand2")
closeextralabel = closeextra.create_text( 19, 19,  text="×", fill="#F5E8D2", font=("Iceland", 25))
closeextra.place(relx=1, x=-52, y=14)
closeextra.bind(  "<Enter>",lambda event: closeextra.itemconfig(closeextralabel, fill="#F0AA60"))
closeextra.bind(  "<Leave>",lambda event: closeextra.itemconfig(closeextralabel, fill="#F5E8D2"))
closeextra.bind("<Button-1>", closeextramenu)
def openextramenu(event=None):
    positionextramenu()
    extraoverlay.deiconify()
    extraoverlay.lift()
    extradialog.deiconify()
    extradialog.lift()
    extradialog.focus_force()
extraoverlay.bind("<Button-1>", closeextramenu)
extradialog.bind("<Escape>", closeextramenu)
app.bind("<Configure>", positionextramenu, add="+")
for item in (extrabox, extratext):
    canvas.tag_bind(item, "<Button-1>", openextramenu)
app.after_idle(initialize_renderer)#
app.mainloop()


