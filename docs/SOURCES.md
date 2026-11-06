# Sources

Original register, 2 October 2026: opened (full text read, with the version named), abstract only,
or not opened. Records were corrected against Crossref. Formulations cite opened sources.


## Formulations and algorithms

| Source | Status | Use |
| --- | --- | --- |
| Chang, T.-J., Meade, N., Beasley, J. E. and Sharaiha, Y. M. (2000), "Heuristics for cardinality constrained portfolio optimisation", *Computers & Operations Research* 27(13), 1271–1302, doi:10.1016/S0305-0548(99)00074-X | opened: authors' preprint, May 1998, revised February 1999 | F1, eqs. (5)–(13); $\lambda = (e-1)/(E-1)$, $E = 50$; test data; Table 4 heuristic errors; sec. 5.2.2 error measure |
| Bienstock, D. (1996), "Computational study of a family of mixed-integer quadratic programming problems", *Mathematical Programming* 74(2), 121–140, doi:10.1007/BF02592208 | abstract only | context for F1; no row taken from it |
| Frangioni, A. and Gentile, C. (2006), "Perspective cuts for a class of convex 0–1 mixed integer programs", *Mathematical Programming* 106(2), 225–236, doi:10.1007/s10107-005-0594-3 | opened: authors' manuscript | F1p; $D = \lambda_{\min} I$ |
| Günlük, O. and Linderoth, J. (2010), "Perspective reformulations of mixed integer nonlinear programs with indicator variables", *Mathematical Programming* 124(1–2), 183–205, doi:10.1007/s10107-010-0360-z | opened: technical report, 2 June 2009 | F1p, eqs. (17)–(23) |
| Westerlund, T. and Pettersson, F. (1995), "An extended cutting plane method for solving convex MINLP problems", *Computers & Chemical Engineering* 19(Suppl.), S131–S136, doi:10.1016/0098-1354(95)00164-W | opened | G2 pins (`tests/models.rs`), eqs. (9)–(15); the method is no longer maintained |
| Konno, H. and Yamazaki, H. (1991), "Mean-absolute deviation portfolio optimization model and its applications to Tokyo stock market", *Management Science* 37(5), 519–531, doi:10.1287/mnsc.37.5.519 | opened (scanned pages 520–524) | F2, eqs. (3.2)–(3.6) |
| Rockafellar, R. T. and Uryasev, S. (2000), "Optimization of conditional value-at-risk", *Journal of Risk* 2(3), 21–41, doi:10.21314/JOR.2000.038 | opened: author's copy | F3, eq. (17) with (15)–(16); Theorem 1 |
| Canakgoz, N. A. and Beasley, J. E. (2009), "Mixed-integer programming approaches for index tracking and enhanced indexation", *European Journal of Operational Research* 196(1), 384–399, doi:10.1016/j.ejor.2008.03.015 | opened in session 1 through a document mirror; rows transcribed to the working note; not re-opened in session 2 (the mirror no longer served its text). Structure corroborated by Bruni et al. (2015) | F4, eqs. (1)–(11), (16)–(26); Tables 1–2 |
| Bruni, R., Cesarone, F., Scozzari, A. and Tardella, F. (2015), "A linear risk-return model for enhanced indexation in portfolio optimization", *OR Spectrum* 37(3), 735–759, doi:10.1007/s00291-014-0383-6 | opened: authors' copy | independent account of the Canakgoz–Beasley approach and windows |
| Beasley, J. E., Meade, N. and Chang, T.-J. (2003), "An evolutionary heuristic for the index tracking problem", *European Journal of Operational Research* 148(3), 621–643, doi:10.1016/S0377-2217(02)00425-3 | abstract only | origin of indtrack1–5; no number taken |
| Harvey, C. R., Liechty, J. C., Liechty, M. W. and Müller, P. (2010), "Portfolio selection with higher moments", *Quantitative Finance* 10(5), 469–485, doi:10.1080/14697681003756877 | opened | F5, eqs. (3)–(6) |
| Jondeau, E. and Rockinger, M. (2006), "Optimal portfolio allocation under higher moments", *European Financial Management* 12(1), 29–55, doi:10.1111/j.1354-7798.2006.00309.x | opened | F5: expansion about $1+\mu^\top w$, eq. (2) |
| Cesarone, F., Scozzari, A. and Tardella, F. (2013), "A new method for mean-variance portfolio optimization with cardinality constraints", *Annals of Operations Research* 205(1), 213–234, doi:10.1007/s10479-012-1165-7 | abstract only (a 2008 conference paper by the same authors opened; it is not this article) | candidate published exact frontier; not used |
| Bertsimas, D. and Cory-Wright, R. (2022), "A scalable algorithm for sparse portfolio selection", *INFORMS Journal on Computing* 34(3), 1489–1511, doi:10.1287/ijoc.2021.1127 | opened: preprint | context; its eq. (34) carries a ridge term, so its outputs are not F1 targets |

## Estimation and inference

| Source | Status | Use |
| --- | --- | --- |
| Ledoit, O. and Wolf, M. (2004), "Honey, I shrunk the sample covariance matrix", *Journal of Portfolio Management* 30(4), 110–119, doi:10.3905/jpm.2004.110 | opened: working paper, November 2003 (the published scan has no text layer) | `est::shrink`, Appendices A–B, eqs. (2)–(5) |
| Ledoit, O. and Wolf, M. (2008), "Robust performance hypothesis testing with the Sharpe ratio", *Journal of Empirical Finance* 15(5), 850–859, doi:10.1016/j.jempfin.2008.03.002 | opened: working paper 320, January 2008 | `ev::lw`, sec. 3.1, eqs. (1)–(5) |
| Politis, D. N. and Romano, J. P. (1994), "The stationary bootstrap", *Journal of the American Statistical Association* 89(428), 1303–1313, doi:10.1080/01621459.1994.10476870 | opened (scanned) | `ev::sbi` |
| DeMiguel, V., Garlappi, L. and Uppal, R. (2009), "Optimal versus naive diversification: how inefficient is the 1/N portfolio strategy?", *Review of Financial Studies* 22(5), 1915–1953, doi:10.1093/rfs/hhm075 | opened: 2006 NBER Summer Institute version | B2; rolling windows $M \in \{60, 120\}$ |

## Data and empirical finance

| Source | Status | Use |
| --- | --- | --- |
| Beasley, J. E. (1990), "OR-Library: distributing test problems by electronic mail", *Journal of the Operational Research Society* 41(11), 1069–1072, doi:10.1057/jors.1990.166 | abstract only; the OR-Library descriptor and licence pages opened | D1, D2 |
| Bessembinder, H. (2018), "Do stocks outperform Treasury bills?", *Journal of Financial Economics* 129(3), 440–457, doi:10.1016/j.jfineco.2018.06.004 | opened: accepted manuscript | share codes 10–12; delisting returns; eq. (3); bootstrap protocol and Table 4 (G3) |
| Bessembinder, H., Chen, T.-F., Choi, G. and Wei, K. C. J. (2023), "Long-term shareholder returns: evidence from 64,000 global stocks", *Financial Analysts Journal* 79(3), 33–63, doi:10.1080/0015198X.2023.2188870 | opened | out of scope (international extension) |
| Farago, A. and Hjalmarsson, E. (2023), "Long-horizon stock returns are positively skewed", *Review of Finance* 27(2), 495–538, doi:10.1093/rof/rfac021 | opened: working paper, 16 April 2021 | the compounding mechanism. Pages corrected from 495–532 |
| Shumway, T. (1997), "The delisting bias in CRSP data", *Journal of Finance* 52(1), 327–340, doi:10.1111/j.1540-6261.1997.tb03818.x | opened | −30% fill, NYSE/AMEX codes 500, 520–584 |
| Shumway, T. and Warther, V. A. (1999), "The delisting bias in CRSP's Nasdaq data and its implications for the size effect", *Journal of Finance* 54(6), 2361–2379, doi:10.1111/0022-1082.00192 | opened | −55% fill, Nasdaq codes 500, 505–588 |
| Fama, E. F. and French, K. R. (2004), "New lists: fundamentals and survival rates", *Journal of Financial Economics* 73(2), 229–269, doi:10.1016/j.jfineco.2003.04.001 | abstract only | context for the history filter (RQ4) |
| Kacperczyk, M., Sialm, C. and Zheng, L. (2005), "On the industry concentration of actively managed equity mutual funds", *Journal of Finance* 60(4), 1983–2011, doi:10.1111/j.1540-6261.2005.00785.x | opened | $K = 65$: median stocks held, Table I |
| Cremers, K. J. M. and Petajisto, A. (2009), "How active is your fund manager? A new measure that predicts performance", *Review of Financial Studies* 22(9), 3329–3365, doi:10.1093/rfs/hhp057 | opened: working paper, 7 August 2006 | Active Share (README) |
| Kenneth R. French data library, `F-F_Research_Data_Factors` | opened (file fetched) | D4, RF |
| [Kenneth R. French, 12 Industry Portfolios](https://mba.tuck.dartmouth.edu/pages/faculty/ken.french/Data_Library/det_12_ind_port.html), monthly value-weighted block; [library revision policy](https://mba.tuck.dartmouth.edu/pages/faculty/ken.french/data_library.html) | opened 5 November 2026: official construction/library pages and downloaded 202609-vintage CSV ZIP | Separate industry experiment, docs/INDUSTRY.md; aggregate portfolios, not a substitute for D3 |
| Arko, M. T. (2025), "Stock return underperformance and compositional bias: a replication and extension of Bessembinder (2018)", unpublished manuscript, 15 December 2025 | not opened; cited with the author's permission | context for RQ4 only; no figure used |

## Software

| Source | Status | Use |
| --- | --- | --- |
| Hojny, C. et al. (2025), "The SCIP Optimization Suite 10.0", arXiv:2511.18580 | opened | SCIP 10.0.2, the build bundled by `scip-sys` 0.1.28 |
| [russcip](https://github.com/scipopt/russcip) 0.11.0, Rust interface to SCIP (Apache-2.0) | opened: source of the release used | `src/scip.rs`, `mv::f1`, `sk::f5` |
| Huangfu, Q. and Hall, J. A. J. (2018), "Parallelizing the dual revised simplex method", *Mathematical Programming Computation* 10(1), 119–142, doi:10.1007/s12532-017-0130-5 | abstract only | HiGHS 1.15.0 |
| [highs](https://github.com/rust-or/highs) 2.4.0 and `highs-sys` 1.15.0, Rust interface to HiGHS (MIT) | opened: source of the release used | `src/lp.rs` |

## Prior work and existing software

Two searches on 2 October 2026 found no study evaluating optimal $K$-name portfolios by
Bessembinder's long-horizon measure on a delisting-inclusive universe; no priority is claimed.
Riskfolio-Lib (`Portfolio.py`, `card`) and skfolio (`_mixed_integer_constraints`, a minimum-CVaR
example with `cardinality=5`) implement cardinality constraints already; they are cross-checks,
not dependencies.
