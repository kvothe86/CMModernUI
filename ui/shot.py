"""Save the client area of the running game window as a PNG.

usage: python shot.py out.png
       python shot.py close        asks the game window to close
"""
import ctypes
import sys
from ctypes import wintypes
from PIL import ImageGrab

user32 = ctypes.windll.user32
user32.SetProcessDPIAware()
found = []


@ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
def each(hwnd, _):
    title = ctypes.create_unicode_buffer(256)
    user32.GetWindowTextW(hwnd, title, 256)
    if user32.IsWindowVisible(hwnd) and 'Championship Manager' in title.value:
        found.append(hwnd)
    return True


user32.EnumWindows(each, 0)
if not found:
    sys.exit('no game window')
if sys.argv[1] == 'close':
    user32.PostMessageW(found[0], 0x10, 0, 0)  # WM_CLOSE
    sys.exit()
rect, origin = wintypes.RECT(), wintypes.POINT(0, 0)
user32.GetClientRect(found[0], ctypes.byref(rect))
user32.ClientToScreen(found[0], ctypes.byref(origin))
# the grab takes whatever is on screen there: refuse when the game is minimised or covered
centre = wintypes.POINT(origin.x + rect.right // 2, origin.y + rect.bottom // 2)
user32.WindowFromPoint.argtypes = [wintypes.POINT]
user32.WindowFromPoint.restype = wintypes.HWND
user32.GetAncestor.restype = wintypes.HWND
top = user32.GetAncestor(user32.WindowFromPoint(centre), 2)  # GA_ROOT
if user32.IsIconic(found[0]) or top != found[0]:
    sys.exit('game window is minimised or covered: bring it to the front first')
box = (origin.x, origin.y, origin.x + rect.right, origin.y + rect.bottom)
ImageGrab.grab(bbox=box, all_screens=True).save(sys.argv[1])
print(sys.argv[1], box)
