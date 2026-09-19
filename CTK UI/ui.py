import customtkinter as ctk
from ctypes import windll
from tkinter import Canvas
import sys
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
app = ctk.CTk()
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
def update_shadow(width, height):
    left =width/2 + 16
    for i, line in enumerate(shadow_lines):
        x = left - 6 + i * 2
        canvas.coords(line, x+16, 0, x+5, 5, x, 16, x, height-16, x+5, height-5, x+16, height)

def resize_cmd_boxes(width):
    box_width = max(200, width / 2 -12) if sidebar.winfo_manager() else width-16
    canvas.itemconfig(command_window, width=box_width)
    canvas.itemconfig(history_window, width=box_width)

def resizethings(event):
    canvas.coords(horizontal, 100, 100, event.width, 100)
    canvas.coords(vertical, 100, 100, 100, event.height)
    resize_cmd_boxes(event.width)
    update_shadow(event.width, event.height)
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
ctk.CTkLabel(sidebar, text="What's on \n your mind today?", font=("Lexend", 16)).pack(padx=15, pady=(20, 10))
ai_input = ctk.CTkEntry(sidebar, placeholder_text="Start typing...", font=("Lexend", 12))
ai_input.place(relx=0.5, rely=1, y=-15, anchor="s", relwidth=0.8)
def toggleai(event=None):
    if sidebar.winfo_manager():
        sidebar.place_forget()
        for line in shadow_lines:
            canvas.itemconfigure(line, state="hidden")
    else:
        sidebar.place(relx=1, x=16, y=0, anchor="ne",
                      relwidth=0.5, relheight=1)
        update_shadow(app.winfo_width(), app.winfo_height())
        for line in shadow_lines:
            canvas.itemconfigure(line, state="normal")
    resize_cmd_boxes(canvas.winfo_width())
close_canvas = Canvas(sidebar, width=32, height=32, bg="#2C2C2E", highlightthickness=0)
close_canvas.place(x=12, y=12)
close_x = close_canvas.create_text(16, 16, text="×", fill='white', font=("Lexend", 20))
close_canvas.tag_bind(close_x, "<Button-1>", toggleai)
canvas.tag_bind(AI, "<Button-1>", toggleai)


app.mainloop()



