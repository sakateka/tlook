import sys
import time
import math

def signals():
    counter = 3
    while True:
        time.sleep(0.2)
        counter += 0.001
        print("line1={}".format(math.sin(counter / 0.09) * 18))
        print("input-a={}".format(math.sin(counter / 0.09) * 7))
        print("metric-b={}".format(math.sin((counter+0.098) / 0.09) * 25))
        print("input-g2={}".format(math.sin((counter*2) / 0.09) * 3))
        print("graph-line={}".format(math.sin((counter*3) / 0.09) * 5))
        print("input-g9={}".format(math.sin((counter*5) / 0.09) * 5))
        print("yconst-1=10")
        print("yconst-2=20")
        print("yconst-3=23")
        print("yconst-4=0")
        sys.stdout.flush()

try:
    signals()
except:
    pass
