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
history_window = canvas.create_window(8, 10, window=history, anchor='nw', height=54)
history.configure(state="disabled")
command_window = canvas.create_window(8, 70, window=command, anchor='nw', height=20)
horizontal = canvas.create_line(100, 100, 1200, 100, fill="#1D1B1B", width=3)
vertical = canvas.create_line(100, 100, 100, 850)

def resizethings(event):
    canvas.coords(horizontal, 100, 100, event.width, 100)
    canvas.coords(vertical, 100, 100, 100, event.height)
    canvas.itemconfigure(command_window, width=max(200, event.width-16))
    canvas.itemconfigure(history_window, width=event.width - 16)
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






app.mainloop()



