import customtkinter as ctk
from ctypes import windll
from tkinter import Canvas
from pathlib import Path

ctk.set_appearance_mode('dark')
fontpath = Path(__file__).resolve().parent/"Assets"/"Lexend-Regular.ttf"
windll.gdi32.AddFontResourceExW(str(fontpath), 0x10, 0)

app = ctk.CTk()
app.title("Armor 3D")
app.geometry("1100x700")
app.minsize(850, 500)

canvas = Canvas(app, bg="#202326", highlightthickness=0)
canvas.pack(fill='both', expand=True)
horizontal = canvas.create_line(100, 100, 1200, 100, fill="#1D1B1B", width=3)
vertical = canvas.create_line(100, 100, 100, 850)
def resizelines(event):
    canvas.coords(horizontal, 100, 100, event.width, 100)
    canvas.coords(vertical, 100, 100, 100, event.height)
canvas.bind("<Configure>", resizelines)


app.mainloop()



