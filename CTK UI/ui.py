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
canvas = Canvas(app, bg="#252729", highlightthickness=0)
canvas.pack(fill='both', expand=True)
command = ctk.CTkEntry(canvas, placeholder_text="Command:", font=("Lexend", 12), fg_color="#2C2C2E", border_color="#1D1B1B")
history = ctk.CTkTextbox( canvas, font=("Lexend", 11), fg_color="#2C2C2E", border_color="#1D1B1B",  border_width=2)
history_window = canvas.create_window(8, 19, window=history, anchor='nw', height=45)
history.configure(state="disabled")
command_window = canvas.create_window(8, 70, window=command, anchor='nw', height=20)
horizontal = canvas.create_line(100, 100, 1200, 100, fill="#1D1B1B", width=3)
vertical = canvas.create_line(100, 100, 100, 850)

shadow_lines= [canvas.create_line(0, 0, 0, 0, fill=color, width=2,  smooth=True, splinesteps=20, state="hidden") for color in ("#222426", "#1D1F21", "#17191B")]
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
#         canvas.create_line(x, 101, x, height, fill="#3A3D40", tags="viewport_grid")
#         x += spacing
#     y = 101 + pan_y % spacing
#     while y < height:
#         canvas.create_line(101, y, width, y, fill="#3A3D40", tags="viewport_grid")
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

filez = canvas.create_text(24, 8, text="File", font=("Lexend", 8), fill='#ffffff')
canvas.tag_bind(filez, "<Enter>", lambda event: canvas.itemconfig(filez, fill="#5c5b5b"))
canvas.tag_bind(filez, "<Leave>", lambda event: canvas.itemconfig(filez, fill="white"))

importz = canvas.create_text(68, 8, text='Import', font=("Lexend", 8), fill='white')
canvas.tag_bind(importz, "<Enter>", lambda event: canvas.itemconfig(importz, fill='#5c5b5b'))
canvas.tag_bind(importz, "<Leave>", lambda event: canvas.itemconfig(importz, fill='white'))

analyze = canvas.create_text(120, 8, text='Analyze', font=("Lexend", 8), fill='white')
canvas.tag_bind(analyze, "<Enter>", lambda event: canvas.itemconfig(analyze, fill='#5c5b5b'))
canvas.tag_bind(analyze, "<Leave>", lambda event: canvas.itemconfig(analyze, fill='white'))

tools = canvas.create_text(170, 8, text="Tools", font=("Lexend", 8), fill='white')
canvas.tag_bind(tools, "<Enter>", lambda event: canvas.itemconfig(tools, fill='#5c5b5b'))
canvas.tag_bind(tools, "<Leave>", lambda event: canvas.itemconfig(tools, fill='white'))

AI  = canvas.create_text(226, 8, text='AI Creation', font=("Lexend", 8), fill='white')
canvas.tag_bind(AI, "<Enter>", lambda event: canvas.itemconfig(AI, fill='#5c5b5b'))
canvas.tag_bind(AI, "<Leave>", lambda event: canvas.itemconfig(AI, fill='white'))


sidebar = ctk.CTkFrame(app, width=520, corner_radius=16, fg_color="#2C2C2E", border_color="#191A1C", border_width=4)
sidebar.pack_propagate(False)
prompt_label = ctk.CTkLabel(sidebar, text="", font=("Iceland", 35), width=300, height=82, justify='center')
prompt_label.pack(pady=(24, 10))
ai_input = ctk.CTkEntry(sidebar, placeholder_text="Start typing...", font=("Lexend", 12))
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
close_canvas = Canvas(sidebar, width=32, height=32, bg="#2C2C2E", highlightthickness=0)
close_canvas.place(x=12, y=12)
close_x = close_canvas.create_text(16, 16, text="×", fill='white', font=("Lexend", 20))
close_canvas.tag_bind(close_x, "<Button-1>", toggleai)
canvas.tag_bind(AI, "<Button-1>", toggleai)
resize_handle = Canvas(sidebar, width=12, bg="#2C2C2E", highlightthickness=0, cursor="sb_h_double_arrow")
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
    canvas.itemconfig(polyline_square, fill="#393D40"if hovering else  "", outline="#596066" if hovering else "")
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
    canvas.itemconfig(curve_square, fill='#393D40' if hovering else '', outline="#596066" if hovering else "")
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
    canvas.itemconfig(puzzle_square, fill='#393D40' if hovering else "", outline="#596066" if hovering else "")
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
    canvas.itemconfig(explode_square, fill='#393D40' if hovering else "", outline="#596066" if hovering else "")
    canvas.itemconfig(explode_icon, image=explode_hover if hovering else explode_normal)
canvas.bind("<Motion>", explode_motion, add="+")


app.mainloop()



