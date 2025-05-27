#!/bin/sh
# Convert `test-pattern.png` into a greyscale PNG file.
pngtopnm test-pattern.png | ppmtopgm | pnmtopng >test-pattern-grey.png
