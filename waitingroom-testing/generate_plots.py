import pandas as pd
import matplotlib.pyplot as plt
import numpy as np

df = pd.read_csv('out.csv', sep='\t')

print(df)
"""
      target  total  killed  kendall_tau     time_taken
0        100    100       0     0.000751  100276.276000
1        100    100       1     0.000765  100323.808809
2        100    100       5     0.000843  100435.563000
3        100   2500       0     0.000032  101191.329000
4        100   2500       1     0.000037  101238.496000
5        100   2500       5     0.000056  101586.045000
6        100    500       0     0.000157  100600.669000
7        100    500       1     0.000159  100640.430000
8        100    500       5     0.000178  100739.547000
9         20    100       0     0.000751  100276.276000
10        20    100       1     0.000765  100323.808809
11        20    100       5     0.000843  100435.563000
12        20   2500       0     0.000030  137183.313000
13        20   2500       1     0.000032  139685.012000
14        20   2500       5     0.000039  145618.758000
15        20    500       0     0.000157  100600.669000
16        20    500       1     0.000159  100642.055000
17        20    500       5     0.000177  100743.646000
18  99999999    100       0     0.000751  100276.276000
19  99999999    100       1     0.000765  100323.808809
20  99999999    100       5     0.000843  100435.563000
21  99999999   2500       0     0.000032  101191.329000
22  99999999   2500       1     0.000037  101240.843000
23  99999999   2500       5     0.000058  101595.620000
24  99999999    500       0     0.000157  100600.669000
25  99999999    500       1     0.000159  100640.430000
26  99999999    500       5     0.000178  100739.547000
"""

# Plotting

# Now we plot the kendall tau distances.
# Use different graphs for the different total values


for total in df['total'].unique():
    fig, ax = plt.subplots()    
    for target in df['target'].unique():
        data = df[(df['target'] == target) & (df['total'] == total)]
        ax.plot(data['killed'], data['kendall_tau'], label=f'target num of users={target}', linestyle='', marker='o', alpha=0.5)
    ax.set_xlabel('Number of nodes added/removed')
    ax.set_ylabel('Average Kendall Tau Distance')
    ax.legend()
    plt.title(f'Kendall Tau vs #nodes added/removed with users={total}')
    plt.tight_layout()
    plt.savefig(f'kendall_tau_total_{total}.png')


