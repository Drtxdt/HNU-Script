import sympy
from sympy.logic.boolalg import SOPform, POSform
import matplotlib.pyplot as plt
import numpy as np
from matplotlib.patches import Rectangle
import itertools


def get_user_input():
    # Ask for number of variables
    n = int(input("请输入变量的数量（支持范围：2 ~ 4，卡诺图支持：4）:"))
    if n < 2 or n > 4:
        raise ValueError("仅支持2 ~ 4个变量")

    # Variable names
    vars = sympy.symbols(' '.join([chr(65 + i) for i in range(n)]))  # A B C D etc.

    # Ask for type: sum of minterms or product of maxterms
    expr_type = input("如果是Σm (minterms) 请输入 'sum' | 如果是ΠM (maxterms) 请输入'product': ").strip().lower()

    # Ask for the terms
    terms_str = input("请输入函数括号中的值(整数，使用英文逗号分隔): ")
    terms = [int(t.strip()) for t in terms_str.split(',')]

    # Ask for don't cares
    has_dc = input("有没有无关项？(y/n): ").strip().lower()
    dc = []
    if has_dc == 'y':
        dc_str = input("请输入无关项(整数，使用英文逗号分隔): ")
        dc = [int(t.strip()) for t in dc_str.split(',')]

    # If product of maxterms, convert to minterms (the 1's are where maxterms are 0)
    if expr_type == 'product':
        all_minterms = list(range(2 ** n))
        minterms = [m for m in all_minterms if m not in terms]
        print(f"将最大项转换为最小项: {minterms}")
    else:
        minterms = terms

    return n, vars, minterms, dc


def simplify_expression(n, vars, minterms, dc):
    # Simplify to SOP
    sop = SOPform(vars, minterms, dc)

    # Simplify to POS
    all_terms = set(range(2 ** n))
    zeros = list(all_terms - set(minterms) - set(dc))
    pos = POSform(vars, zeros, dc)

    return sop, pos


def get_term_key_sop(term, var_index):
    if term.func == sympy.logic.boolalg.And:
        literals = term.args
    else:
        literals = [term]
    key = 0
    for lit in literals:
        if lit.func != sympy.logic.boolalg.Not:
            key += 2 ** var_index[lit]
    return key


def get_term_key_pos(term, var_index):
    if term.func == sympy.logic.boolalg.Or:
        literals = term.args
    else:
        literals = [term]
    key = 0
    for lit in literals:
        if lit.func == sympy.logic.boolalg.Not:
            key += 2 ** var_index[lit.args[0]]
    return key


def sort_literals(term, is_sop=True):
    if is_sop:
        if term.func == sympy.logic.boolalg.And:
            sorted_args = sorted(term.args, key=str)
            return sympy.And(*sorted_args)
        else:
            return term
    else:
        if term.func == sympy.logic.boolalg.Or:
            sorted_args = sorted(term.args, key=str)
            return sympy.Or(*sorted_args)
        else:
            return term


def plot_kmap(n, vars, minterms, dc, sop):
    if n != 4:
        print("卡诺图的绘制仅支持4个变量。")
        return

    # For 4 variables, assume vars A B C D, row AB, col CD
    row_labels = ['00', '01', '11', '10']  # Gray code for AB
    col_labels = ['00', '01', '11', '10']  # Gray code for CD

    binary_to_gray_index = {0: 0, 1: 1, 2: 3, 3: 2}

    kmap = np.full((4, 4), '0', dtype='<U1')
    for i in range(16):
        ab = i // 4
        cd = i % 4
        row = binary_to_gray_index[ab]
        col = binary_to_gray_index[cd]
        if i in minterms:
            kmap[row, col] = '1'
        elif i in dc:
            kmap[row, col] = 'd'

    fig, ax = plt.subplots(figsize=(6, 6))
    ax.set_xlim(0, 4)
    ax.set_ylim(0, 4)
    ax.axis('off')

    # Draw grid lines
    for i in range(5):
        ax.hlines(i, 0, 4, colors='black')
        ax.vlines(i, 0, 4, colors='black')

    # Add variable labels in top-left corner
    row_vars_str = f"{vars[0]}{vars[1]}"
    col_vars_str = f"{vars[2]}{vars[3]}"
    ax.text(-0.5, 4.5, f"{row_vars_str} \\ {col_vars_str}", ha='center', va='center', fontsize=12)

    # Add row labels
    for i in range(4):
        ax.text(-0.5, 3.5 - i, row_labels[i], ha='center', va='center')

    # Add col labels
    for i in range(4):
        ax.text(0.5 + i, 4.2, col_labels[i], ha='center', va='center')

    # Add values
    for row in range(4):
        for col in range(4):
            ax.text(col + 0.5, 3.5 - row, kmap[row, col], ha='center', va='center')

    # Get terms from sop
    if sop.func == sympy.Or:
        terms = sop.args
    else:
        terms = [sop]

    colors = ['red', 'blue', 'green', 'purple', 'orange', 'cyan']  # cycle colors for groups

    for idx, term in enumerate(terms):
        color = colors[idx % len(colors)]

        # Parse term to fixed
        if term.func == sympy.And:
            literals = term.args
        else:
            literals = [term]
        fixed = {}
        for lit in literals:
            if lit.func == sympy.Not:
                v = lit.args[0]
                val = 0
            else:
                v = lit
                val = 1
            fixed[v] = val

        # Row vars A B (0,1)
        row_vars = vars[0:2]
        col_vars = vars[2:4]

        # Get row indices
        row_fixed = {k: v for k, v in fixed.items() if k in row_vars}
        row_free = [v for v in row_vars if v not in row_fixed]
        row_list = []
        for ass in itertools.product([0, 1], repeat=len(row_free)):
            curr = dict(zip(row_free, ass))
            curr.update(row_fixed)
            ab = curr[row_vars[0]] * 2 + curr[row_vars[1]]
            row_index = binary_to_gray_index[ab]
            row_list.append(row_index)

        sorted_row = sorted(set(row_list))
        len_row = len(sorted_row)
        if len_row == 4 or (len_row > 0 and sorted_row[-1] - sorted_row[0] + 1 == len_row):
            row_groups = [[sorted_row[0], len_row]]
        else:
            row_groups = [[r, 1] for r in sorted_row]

        # Get col indices
        col_fixed = {k: v for k, v in fixed.items() if k in col_vars}
        col_free = [v for v in col_vars if v not in col_fixed]
        col_list = []
        for ass in itertools.product([0, 1], repeat=len(col_free)):
            curr = dict(zip(col_free, ass))
            curr.update(col_fixed)
            cd = curr[col_vars[0]] * 2 + curr[col_vars[1]]
            col_index = binary_to_gray_index[cd]
            col_list.append(col_index)

        sorted_col = sorted(set(col_list))
        len_col = len(sorted_col)
        if len_col == 4 or (len_col > 0 and sorted_col[-1] - sorted_col[0] + 1 == len_col):
            col_groups = [[sorted_col[0], len_col]]
        else:
            col_groups = [[c, 1] for c in sorted_col]

        # Draw the rectangles with bolder lines
        for row_start, row_height in row_groups:
            for col_start, col_width in col_groups:
                y_bottom = 4 - (row_start + row_height)
                ax.add_patch(Rectangle((col_start, y_bottom), col_width, row_height,
                                       fill=None, edgecolor=color, lw=4))  # Increased lw to 4 for bolder

    plt.show()


def main():
    n, vars, minterms, dc = get_user_input()
    sop, pos = simplify_expression(n, vars, minterms, dc)

    var_index = {vars[i]: n - 1 - i for i in range(n)}  # A: n-1, ..., MSB

    # Process SOP: sort internals, then sort overall by binary key
    if sop.func == sympy.logic.boolalg.Or:
        terms = sop.args
    else:
        terms = [sop]
    sorted_internal_terms = [sort_literals(t, is_sop=True) for t in terms]
    sorted_terms = sorted(sorted_internal_terms, key=lambda t: get_term_key_sop(t, var_index))
    print("最简与或式 SOP: " + " + ".join(str(t) for t in sorted_terms))

    # Process POS: sort internals, then sort overall by binary key
    if pos.func == sympy.logic.boolalg.And:
        terms = pos.args
    else:
        terms = [pos]
    sorted_internal_terms = [sort_literals(t, is_sop=False) for t in terms]
    sorted_terms = sorted(sorted_internal_terms, key=lambda t: get_term_key_pos(t, var_index))
    print("最简或与式 POS: " + " * ".join("(" + str(t) + ")" for t in sorted_terms))

    plot_kmap(n, vars, minterms, dc, sop)


if __name__ == "__main__":
    main()