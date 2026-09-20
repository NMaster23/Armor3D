import customtkinter as ctk
from ctypes import windll
from tkinter import Canvas
import sys
from PIL import Image, ImageEnhance, ImageTk, ImageDraw
from pathlib import Path
import os
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
app.minsize(850, 500)
canvas = Canvas(app, bg="#242B23", highlightthickness=0)
canvas.pack(fill='both', expand=True)
command = ctk.CTkEntry(canvas, placeholder_text="Command:", font=("Lexend", 12), fg_color="#3B322A", border_color="#70543B")
history = ctk.CTkTextbox( canvas, font=("Lexend", 11), fg_color="#3B322A", border_color="#70543B",  border_width=2)
history_window = canvas.create_window(8, 19, window=history, anchor='nw', height=45)
history.configure(state="disabled")
command_window = canvas.create_window(8, 70, window=command, anchor='nw', height=20)
horizontal = canvas.create_line(100, 100, 1200, 100, fill="#70543B", width=3)
vertical = canvas.create_line(100, 100, 100, 850, fill="#70543B", width=3)

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
def resizethings(event):
    canvas.coords(horizontal, 100, 100, event.width, 100)
    canvas.coords(vertical, 100, 100, 100, event.height)
    resize_cmd_boxes(event.width)
    update_shadow(event.width, event.height, current_offset)
    # draw_grid(event.width, event.height)
canvas.bind("<Configure>", resizethings)

def runcmd(event):
    typed = command.get().strip()
    if not typed:
        return
    history.configure(state='normal')
    history.insert('end', f"> {typed}\nCommand not found\n")
    history.see("end")
    history.configure(state='disabled')
    command.delete(0, 'end')
command.bind("<Return>", runcmd)

filez = canvas.create_text(24, 8, text="File", font=("Lexend", 8), fill='#F5E8D2')
canvas.tag_bind(filez, "<Enter>", lambda event: canvas.itemconfig(filez, fill="#F0AA60"))
canvas.tag_bind(filez, "<Leave>", lambda event: canvas.itemconfig(filez, fill="#F5E8D2"))
filemenu = Canvas(app, width=160, height=136, bg="#3B322A",  highlightthickness=1, highlightbackground="#A66B3E")
menu_rows = []
for i, name in enumerate(("New", "Save", "Save As", "Pumpkin :)")): 
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
file_hover_job = None
def open_file_menu():
    global file_hover_job
    file_hover_job = None
    filemenu.place(x=8, y=20)
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
        filemenu.place_forget()
    else:
        open_file_menu()
fileclosejob = None
def pointeronfilemenu(event):
    x=  event.x_root- app.winfo_rootx()
    y = event.y_root - app.winfo_rooty()
    return (8 <= x <= 45 and 0 <= y <= 20) or (8 <+ x <= 170 and 20 <= y <= 158)
def cancelfileclose():
    global fileclosejob
    if fileclosejob is not None:
        app.after_cancel(fileclosejob)
        fileclosejob = None
def closefilemenu():
    global fileclosejob
    fileclosejob = None
    filemenu.place_forget()
def filepointermtion(event):
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
canvas.tag_bind(analyze, "<Enter>", lambda event: canvas.itemconfig(analyze, fill='#F0AA60'))
canvas.tag_bind(analyze, "<Leave>", lambda event: canvas.itemconfig(analyze, fill='#F5E8D2'))

tools = canvas.create_text(170, 8, text="Tools", font=("Lexend", 8), fill='#F5E8D2')
canvas.tag_bind(tools, "<Enter>", lambda event: canvas.itemconfig(tools, fill='#F0AA60'))
canvas.tag_bind(tools, "<Leave>", lambda event: canvas.itemconfig(tools, fill='#F5E8D2'))

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
def polyline_motion(event):
    hovering = 8 <= event.x<= 52 and 103 <= event.y <= 147
    canvas.itemconfig(polyline_square, fill="#67442F"if hovering else  "", outline="#E28B45" if hovering else "")
    canvas.itemconfig(polyline_icon, image=polyline_hover if hovering else polylinenormal)
canvas.bind("<Motion>", polyline_motion)

curve_image = Image.open(getpath("Assets/curvez.png")).convert("RGBA")
bounds = curve_image.getbbox()
if bounds:
    curve_image = curve_image.crop(bounds)
curve_image.thumbnail((30, 30), Image.Resampling.LANCZOS)
curve_normal = ImageTk.PhotoImage(curve_image)
curve_hover = ImageTk.PhotoImage(ImageEnhance.Brightness(curve_image).enhance(0.6))
curve_square = canvas.create_rectangle(53, 103, 96, 147, fill='', outline='')
curve_icon = canvas.create_image(75, 125, image=curve_normal)
def curve_motion(event):
    hovering = 53 <= event.x <=97 and 103 <= event.y <=147
    canvas.itemconfig(curve_square, fill='#67442F' if hovering else '', outline="#E28B45" if hovering else "")
    canvas.itemconfig(curve_icon, image=curve_hover if hovering else curve_normal)
canvas.bind("<Motion>", curve_motion, add="+")

puzzle_image = Image.open(getpath("Assets/joinz.png")).convert("RGBA")
bounds = puzzle_image.getbbox()
if bounds:
    puzzle_image = puzzle_image.crop(bounds)
puzzle_image.thumbnail((30, 30), Image.Resampling.LANCZOS)
puzzle_normal = ImageTk.PhotoImage(puzzle_image)
puzzle_hover= ImageTk.PhotoImage(ImageEnhance.Brightness(puzzle_image).enhance(0.6))
puzzle_square  = canvas.create_rectangle(8, 148, 52, 192, fill='', outline='')
puzzle_icon = canvas.create_image(30, 170, image=puzzle_normal)
def puzzlemotion(event):
    hovering = 8 <= event.x <= 52 and 148 <= event.y <= 192
    canvas.itemconfig(puzzle_square, fill='#67442F' if hovering else "", outline="#E28B45" if hovering else "")
    canvas.itemconfig(puzzle_icon, image=puzzle_hover if hovering else puzzle_normal)
canvas.bind("<Motion>", puzzlemotion, add="+")

explode_image = Image.open(getpath("Assets/explode.png")).convert("RGBA")
bounds = explode_image.getbbox()
if bounds: 
    explode_image = explode_image.crop(bounds)
explode_image.thumbnail((30, 30), Image.Resampling.LANCZOS)
explode_normal = ImageTk.PhotoImage(explode_image)
explode_hover = ImageTk.PhotoImage(ImageEnhance.Brightness(explode_image).enhance(0.6))
explode_square = canvas.create_rectangle(53, 148, 97, 192, fill='', outline='')
explode_icon = canvas.create_image(75, 170, image=explode_normal)
def explode_motion(event):
    hovering = 53 <= event.x <= 97 and 148 <= event.y <=192
    canvas.itemconfig(explode_square, fill='#67442F' if hovering else "", outline="#E28B45" if hovering else "")
    canvas.itemconfig(explode_icon, image=explode_hover if hovering else explode_normal)
canvas.bind("<Motion>", explode_motion, add="+")

rectangle_image = Image.open(getpath("Assets/rectangle.png")).convert("RGBA")
bounds = rectangle_image.getbbox()
if bounds:
    rectangle_image = rectangle_image.crop(bounds)
rectangle_image.thumbnail((30, 30), Image.Resampling.LANCZOS)
rectangle_normal = ImageTk.PhotoImage(rectangle_image)
rectangle_hover = ImageTk.PhotoImage(ImageEnhance.Brightness(rectangle_image).enhance(0.6))
rectangle_sqaure = canvas.create_rectangle(8, 193, 52, 237, fill='', outline='')
rectangle_icon = canvas.create_image(30, 215, image=rectangle_normal)
def rectangle_motion(event):
    hovering = 8 <= event.x <= 52 and 193 <= event.y <= 237
    canvas.itemconfig(rectangle_sqaure, fill="#67442F" if hovering else "",  outline="#E28B45" if hovering else "")
    canvas.itemconfig(rectangle_icon,  image=rectangle_hover if hovering else rectangle_normal)
canvas.bind("<Motion>", rectangle_motion, add="+")

text_image = Image.open(getpath("Assets/text.png")).convert("RGBA")
bounds = text_image.getbbox()
if bounds:
    text_image = text_image.crop(bounds)
text_image.thumbnail((30, 30), Image.Resampling.LANCZOS)
text_normal = ImageTk.PhotoImage(text_image)
text_hover = ImageTk.PhotoImage(ImageEnhance.Brightness(text_image).enhance(0.6))
text_square = canvas.create_rectangle(53, 193, 97, 237, fill="", outline="")
text_icon = canvas.create_image(75, 215, image=text_normal)
def text_motion(event):
    hovering = 53 <= event.x <= 97 and 193 <= event.y <= 237
    canvas.itemconfig(text_square, fill="#67442F" if hovering else "", outline="#E28B45" if hovering else "")
    canvas.itemconfig(text_icon, image=text_hover if hovering else text_normal)
canvas.bind("<Motion>", text_motion, add="+")

tooltips = [ (8, 103, 52, 147, "Polyline"), (53, 103, 97, 147, "Curve"), (8, 148, 52, 192, "Join"), (53, 148, 97, 192, "Explode"), (8, 193, 52, 237, "Rectangle"), (53, 193, 97, 237, "Text")]
tooltip_job = None
tooltip_target = None
def show_tooltip(name, top):
    global tooltip_job
    tooltip_job = None
    background = canvas.create_rectangle(0, 0, 0, 0, fill="#34291F", outline="#E28B45", tags="tooltip")
    label = canvas.create_text(111, top+22, text=name, anchor='w', fill='white', font=("Iceland", 11), tags='tooltip')
    x1, y1, x2, y2 = canvas.bbox(label)
    canvas.coords(background, x1-7, y1-5, x2+7, y2+5)
    canvas.tag_raise('tooltip')
def hide_tooltip(event=None):
    global tooltip_job, tooltip_target
    if tooltip_job is not None:
        app.after_cancel(tooltip_job)
        tooltip_job = None
    tooltip_target = None
    canvas.delete("tooltip")
def tooltipmotion(event):
    global tooltip_job, tooltip_target
    target = next(((name, top) for left, top, right, bottom, name in tooltips if left <= event.x <= right and top <= event.y <= bottom), None)
    if target == tooltip_target:
        return
    hide_tooltip()
    tooltip_target = target
    if target is not None:
        tooltip_job = app.after(650, lambda: show_tooltip(*target))
canvas.bind("<Motion>", tooltipmotion, add="+")
canvas.bind("<Leave>", hide_tooltip)



app.mainloop()



