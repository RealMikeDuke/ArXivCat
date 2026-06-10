import os
import sys

if getattr(sys, "frozen", False) and hasattr(sys, "_MEIPASS"):
    os.environ["TCL_LIBRARY"] = os.path.join(sys._MEIPASS, "_tcl_data")
    os.environ["TK_LIBRARY"] = os.path.join(sys._MEIPASS, "_tk_data")
