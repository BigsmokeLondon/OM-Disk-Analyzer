#!/usr/bin/env python3
"""OM Disk Analyzer Professional - by OM Industries Inc.

Run:  python main.py [folder]
      python main.py --screenshot out.png [--tab details] [folder]
"""
import sys

from omdisk_app import main

if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
