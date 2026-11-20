"""Old name kept for convenience: builds everything via tools/build_all.py."""
import os
import runpy

runpy.run_path(os.path.join(os.path.dirname(os.path.abspath(__file__)), "build_all.py"), run_name="__main__")
