// Copyright (C) 1988-1994 Sun Microsystems, Inc. 2550 Garcia Avenue
// Mountain View, California  94043 All rights reserved.
//
// Any person is hereby authorized to download, copy, use, create bug fixes,
// and distribute, subject to the following conditions:
//
// 	1.  the software may not be redistributed for a fee except as
// 	    reasonable to cover media costs;
// 	2.  any copy of the software must include this notice, as well as
// 	    any other embedded copyright notices; and
// 	3.  any distribution of this software or derivative works thereof
// 	    must comply with all applicable U.S. export control laws.
//
// THE SOFTWARE IS MADE AVAILABLE "AS IS" AND WITHOUT EXPRESS OR IMPLIED
// WARRANTY OF ANY KIND, INCLUDING BUT NOT LIMITED TO THE IMPLIED
// WARRANTIES OF DESIGN, MERCHANTIBILITY, FITNESS FOR A PARTICULAR
// PURPOSE, NON-INFRINGEMENT, PERFORMANCE OR CONFORMANCE TO
// SPECIFICATIONS.
//
// BY DOWNLOADING AND/OR USING THIS SOFTWARE, THE USER WAIVES ALL CLAIMS
// AGAINST SUN MICROSYSTEMS, INC. AND ITS AFFILIATED COMPANIES IN ANY
// JURISDICTION, INCLUDING BUT NOT LIMITED TO CLAIMS FOR DAMAGES OR
// EQUITABLE RELIEF BASED ON LOSS OF DATA, AND SPECIFICALLY WAIVES EVEN
// UNKNOWN OR UNANTICIPATED CLAIMS OR LOSSES, PRESENT AND FUTURE.
//
// IN NO EVENT WILL SUN MICROSYSTEMS, INC. OR ANY OF ITS AFFILIATED
// COMPANIES BE LIABLE FOR ANY LOST REVENUE OR PROFITS OR OTHER SPECIAL,
// INDIRECT AND CONSEQUENTIAL DAMAGES, EVEN IF IT HAS BEEN ADVISED OF THE
// POSSIBILITY OF SUCH DAMAGES.
//
// This file is provided with no support and without any obligation on the
// part of Sun Microsystems, Inc. ("Sun") or any of its affiliated
// companies to assist in its use, correction, modification or
// enhancement.  Nevertheless, and without creating any obligation on its
// part, Sun welcomes your comments concerning the software and requests
// that they be sent to fdlibm-comments@sunpro.sun.com.
// sinhd(log(2*max)chopped) is finite, overflow threshold
T(RN,    0x1.633ce8fb9f87dp+9, 0x1.ffffffffffd3bp+1023,   0x1.a6b164p-4, INEXACT)
T(RN,   -0x1.633ce8fb9f87dp+9,-0x1.ffffffffffd3bp+1023,  -0x1.a6b164p-4, INEXACT)
T(RZ,    0x1.633ce8fb9f87dp+9, 0x1.ffffffffffd3ap+1023,  -0x1.cb29d4p-1, INEXACT)
T(RZ,   -0x1.633ce8fb9f87dp+9,-0x1.ffffffffffd3ap+1023,   0x1.cb29d4p-1, INEXACT)
T(RU,    0x1.633ce8fb9f87dp+9, 0x1.ffffffffffd3bp+1023,   0x1.a6b164p-4, INEXACT)
T(RU,   -0x1.633ce8fb9f87dp+9,-0x1.ffffffffffd3ap+1023,   0x1.cb29d4p-1, INEXACT)
T(RD,    0x1.633ce8fb9f87dp+9, 0x1.ffffffffffd3ap+1023,  -0x1.cb29d4p-1, INEXACT)
T(RD,   -0x1.633ce8fb9f87dp+9,-0x1.ffffffffffd3bp+1023,  -0x1.a6b164p-4, INEXACT)
T(RN,    0x1.633ce8fb9f87ep+9,                     inf,          0x0p+0, INEXACT|OVERFLOW)
T(RN,   -0x1.633ce8fb9f87ep+9,                    -inf,          0x0p+0, INEXACT|OVERFLOW)
T(RZ,    0x1.633ce8fb9f87ep+9, 0x1.fffffffffffffp+1023,         -0x1p+0, INEXACT|OVERFLOW)
T(RZ,   -0x1.633ce8fb9f87ep+9,-0x1.fffffffffffffp+1023,          0x1p+0, INEXACT|OVERFLOW)
T(RU,    0x1.633ce8fb9f87ep+9,                     inf,          0x0p+0, INEXACT|OVERFLOW)
T(RU,   -0x1.633ce8fb9f87ep+9,-0x1.fffffffffffffp+1023,          0x1p+0, INEXACT|OVERFLOW)
T(RD,    0x1.633ce8fb9f87ep+9, 0x1.fffffffffffffp+1023,         -0x1p+0, INEXACT|OVERFLOW)
T(RD,   -0x1.633ce8fb9f87ep+9,                    -inf,          0x0p+0, INEXACT|OVERFLOW)
// sinhd(tiny) :=: tiny
T(RN,                 0x1p-67,                 0x1p-67,          0x0p+0, INEXACT)
T(RN,                -0x1p-67,                -0x1p-67,          0x0p+0, INEXACT)
T(RN,               0x1p-1022,               0x1p-1022,          0x0p+0, INEXACT)
T(RN,              -0x1p-1022,              -0x1p-1022,          0x0p+0, INEXACT)
T(RN,               0x1p-1042,               0x1p-1042,          0x0p+0, INEXACT|UNDERFLOW)
T(RN,              -0x1p-1042,              -0x1p-1042,          0x0p+0, INEXACT|UNDERFLOW)
T(RN,               0x1p-1074,               0x1p-1074,          0x0p+0, INEXACT|UNDERFLOW)
T(RN,              -0x1p-1074,              -0x1p-1074,          0x0p+0, INEXACT|UNDERFLOW)
// sinhd(+-0) = +-0
T(RN,                  0x0p+0,                  0x0p+0,          0x0p+0, 0)
T(RN,                 -0x0p+0,                 -0x0p+0,          0x0p+0, 0)
T(RZ,                  0x0p+0,                  0x0p+0,          0x0p+0, 0)
T(RZ,                 -0x0p+0,                 -0x0p+0,          0x0p+0, 0)
T(RU,                  0x0p+0,                  0x0p+0,          0x0p+0, 0)
T(RU,                 -0x0p+0,                 -0x0p+0,          0x0p+0, 0)
T(RD,                  0x0p+0,                  0x0p+0,          0x0p+0, 0)
T(RD,                 -0x0p+0,                 -0x0p+0,          0x0p+0, 0)
// random arguments between -30 30
T(RN,   -0x1.01f5cb2b5006dp+3,  -0x1.8c28619d32c08p+10,   -0x1.e5b07p-3, INEXACT)
T(RN,    0x1.55de4fb825911p+4,   0x1.c5bef10311486p+29,   0x1.d6225ap-2, INEXACT)
T(RN,    0x1.a69db09de7505p+4,   0x1.13a2f3f7db55bp+37,  -0x1.819a24p-3, INEXACT)
T(RN,   -0x1.40920fba96889p+4,  -0x1.df7c90f0e645ap+27,   0x1.139e96p-4, INEXACT)
T(RN,   -0x1.04112e27084ddp+3,   -0x1.a71eb14c98b3p+10,  -0x1.5cd8a2p-6, INEXACT)
T(RN,   -0x1.2dc321b093c41p+0,   -0x1.78a9a1930d4d1p+0,   0x1.a83538p-2, INEXACT)
T(RN,    0x1.15995d18455f5p+4,   0x1.058065e10a9f3p+24,    0x1.b22c5p-4, INEXACT)
T(RN,    0x1.9a7144a51b239p+4,   0x1.0198176da4c49p+36,  -0x1.e8cfbcp-2, INEXACT)
T(RN,   -0x1.42b131079de4dp+2,   -0x1.3590d1df8ba23p+6,   -0x1.22a04p-2, INEXACT)
T(RN,   -0x1.cbda03103b871p+4,  -0x1.6124348c6ad56p+40,   0x1.05e9fep-6, INEXACT)
// sinhd(nan) is nan , sinhd(+-inf) is +-inf
T(RN,                     nan,                     nan,          0x0p+0, 0)
T(RN,                     inf,                     inf,          0x0p+0, 0)
T(RN,                    -inf,                    -inf,          0x0p+0, 0)
T(RD,                     inf,                     inf,          0x0p+0, 0)
T(RD,                    -inf,                    -inf,          0x0p+0, 0)
T(RD,               0x1p-1022,               0x1p-1022,          0x0p+0, INEXACT)
T(RD, 0x1.0000000000001p-1022, 0x1.0000000000001p-1022,          0x0p+0, INEXACT)
T(RD, 0x1.0000000000002p-1022, 0x1.0000000000002p-1022,          0x0p+0, INEXACT)
T(RD,               0x1p-1021,               0x1p-1021,          0x0p+0, INEXACT)
T(RD,               0x1p-1020,               0x1p-1020,          0x0p+0, INEXACT)
T(RD,                 0x1p-26,                 0x1p-26,  -0x1.555556p-3, INEXACT)
T(RD,               0x1.8p-26,               0x1.8p-26,       -0x1.2p-1, INEXACT)
T(RD,              -0x1p-1022,-0x1.0000000000001p-1022,         -0x1p+0, INEXACT)
T(RD,-0x1.0000000000001p-1022,-0x1.0000000000002p-1022,         -0x1p+0, INEXACT)
T(RD,-0x1.0000000000002p-1022,-0x1.0000000000003p-1022,         -0x1p+0, INEXACT)
T(RD,              -0x1p-1021,-0x1.0000000000001p-1021,         -0x1p+0, INEXACT)
T(RD,              -0x1p-1020,-0x1.0000000000001p-1020,         -0x1p+0, INEXACT)
T(RD,               0x1p-1074,               0x1p-1074,          0x0p+0, INEXACT|UNDERFLOW)
T(RD,               0x1p-1073,               0x1p-1073,          0x0p+0, INEXACT|UNDERFLOW)
T(RD,               0x1p-1024,               0x1p-1024,          0x0p+0, INEXACT|UNDERFLOW)
T(RD,               0x1p-1023,               0x1p-1023,          0x0p+0, INEXACT|UNDERFLOW)
T(RD, 0x1.ffffffffffffcp-1023, 0x1.ffffffffffffcp-1023,          0x0p+0, INEXACT|UNDERFLOW)
T(RD, 0x1.ffffffffffffep-1023, 0x1.ffffffffffffep-1023,          0x0p+0, INEXACT|UNDERFLOW)
T(RD,              -0x1p-1074,              -0x1p-1073,         -0x1p+0, INEXACT|UNDERFLOW)
T(RD,              -0x1p-1073,            -0x1.8p-1073,         -0x1p+0, INEXACT|UNDERFLOW)
T(RD,              -0x1p-1024,-0x1.0000000000004p-1024,         -0x1p+0, INEXACT|UNDERFLOW)
T(RD,              -0x1p-1023,-0x1.0000000000002p-1023,         -0x1p+0, INEXACT|UNDERFLOW)
T(RD,-0x1.ffffffffffffcp-1023,-0x1.ffffffffffffep-1023,         -0x1p+0, INEXACT|UNDERFLOW)
T(RD,-0x1.ffffffffffffep-1023,              -0x1p-1022,         -0x1p+0, INEXACT|UNDERFLOW)
T(RD,              0x1.634p+9, 0x1.fffffffffffffp+1023,         -0x1p+0, INEXACT|OVERFLOW)
T(RD,               0x1p+1022, 0x1.fffffffffffffp+1023,         -0x1p+0, INEXACT|OVERFLOW)
T(RD,               0x1p+1023, 0x1.fffffffffffffp+1023,         -0x1p+0, INEXACT|OVERFLOW)
T(RD, 0x1.ffffffffffffep+1023, 0x1.fffffffffffffp+1023,         -0x1p+0, INEXACT|OVERFLOW)
T(RD, 0x1.fffffffffffffp+1023, 0x1.fffffffffffffp+1023,         -0x1p+0, INEXACT|OVERFLOW)
T(RD,             -0x1.634p+9,                    -inf,          0x0p+0, INEXACT|OVERFLOW)
T(RD,              -0x1p+1022,                    -inf,          0x0p+0, INEXACT|OVERFLOW)
T(RD,              -0x1p+1023,                    -inf,          0x0p+0, INEXACT|OVERFLOW)
T(RD,-0x1.ffffffffffffep+1023,                    -inf,          0x0p+0, INEXACT|OVERFLOW)
T(RD,-0x1.fffffffffffffp+1023,                    -inf,          0x0p+0, INEXACT|OVERFLOW)
T(RD,                     nan,                     nan,          0x0p+0, 0)
T(RD,                     nan,                     nan,          0x0p+0, 0)
T(RD,                 0x1p-25,                 0x1p-25,  -0x1.555556p-1, INEXACT)
T(RD,               0x1.4p-25,   0x1.4000000000001p-25,  -0x1.355556p-2, INEXACT)
T(RD,                -0x1p-26,  -0x1.0000000000001p-26,  -0x1.aaaaaap-1, INEXACT)
T(RD,              -0x1.8p-26,  -0x1.8000000000001p-26,       -0x1.cp-2, INEXACT)
T(RD,                -0x1p-25,  -0x1.0000000000001p-25,  -0x1.555556p-2, INEXACT)
T(RD,              -0x1.4p-25,  -0x1.4000000000002p-25,  -0x1.655556p-1, INEXACT)
T(RN, 0x1.0000000000001p-1022, 0x1.0000000000001p-1022,          0x0p+0, INEXACT)
T(RN, 0x1.0000000000002p-1022, 0x1.0000000000002p-1022,          0x0p+0, INEXACT)
T(RN,               0x1p-1021,               0x1p-1021,          0x0p+0, INEXACT)
T(RN,               0x1p-1020,               0x1p-1020,          0x0p+0, INEXACT)
T(RN,                 0x1p-26,                 0x1p-26,  -0x1.555556p-3, INEXACT)
T(RN,                 0x1p-25,   0x1.0000000000001p-25,   0x1.555556p-2, INEXACT)
T(RN,               0x1.4p-25,   0x1.4000000000001p-25,  -0x1.355556p-2, INEXACT)
T(RN,-0x1.0000000000001p-1022,-0x1.0000000000001p-1022,          0x0p+0, INEXACT)
T(RN,-0x1.0000000000002p-1022,-0x1.0000000000002p-1022,          0x0p+0, INEXACT)
T(RN,              -0x1p-1021,              -0x1p-1021,          0x0p+0, INEXACT)
T(RN,              -0x1p-1020,              -0x1p-1020,          0x0p+0, INEXACT)
T(RN,                -0x1p-26,                -0x1p-26,   0x1.555556p-3, INEXACT)
T(RN,                -0x1p-25,  -0x1.0000000000001p-25,  -0x1.555556p-2, INEXACT)
T(RN,              -0x1.4p-25,  -0x1.4000000000001p-25,   0x1.355556p-2, INEXACT)
T(RN,               0x1p-1073,               0x1p-1073,          0x0p+0, INEXACT|UNDERFLOW)
T(RN,               0x1p-1024,               0x1p-1024,          0x0p+0, INEXACT|UNDERFLOW)
T(RN,               0x1p-1023,               0x1p-1023,          0x0p+0, INEXACT|UNDERFLOW)
T(RN, 0x1.ffffffffffffcp-1023, 0x1.ffffffffffffcp-1023,          0x0p+0, INEXACT|UNDERFLOW)
T(RN, 0x1.ffffffffffffep-1023, 0x1.ffffffffffffep-1023,          0x0p+0, INEXACT|UNDERFLOW)
T(RN,              -0x1p-1073,              -0x1p-1073,          0x0p+0, INEXACT|UNDERFLOW)
T(RN,              -0x1p-1024,              -0x1p-1024,          0x0p+0, INEXACT|UNDERFLOW)
T(RN,              -0x1p-1023,              -0x1p-1023,          0x0p+0, INEXACT|UNDERFLOW)
T(RN,-0x1.ffffffffffffcp-1023,-0x1.ffffffffffffcp-1023,          0x0p+0, INEXACT|UNDERFLOW)
T(RN,-0x1.ffffffffffffep-1023,-0x1.ffffffffffffep-1023,          0x0p+0, INEXACT|UNDERFLOW)
T(RN,              0x1.634p+9,                     inf,          0x0p+0, INEXACT|OVERFLOW)
T(RN,               0x1p+1022,                     inf,          0x0p+0, INEXACT|OVERFLOW)
T(RN,               0x1p+1023,                     inf,          0x0p+0, INEXACT|OVERFLOW)
T(RN, 0x1.ffffffffffffep+1023,                     inf,          0x0p+0, INEXACT|OVERFLOW)
T(RN, 0x1.fffffffffffffp+1023,                     inf,          0x0p+0, INEXACT|OVERFLOW)
T(RN,             -0x1.634p+9,                    -inf,          0x0p+0, INEXACT|OVERFLOW)
T(RN,              -0x1p+1022,                    -inf,          0x0p+0, INEXACT|OVERFLOW)
T(RN,              -0x1p+1023,                    -inf,          0x0p+0, INEXACT|OVERFLOW)
T(RN,-0x1.ffffffffffffep+1023,                    -inf,          0x0p+0, INEXACT|OVERFLOW)
T(RN,-0x1.fffffffffffffp+1023,                    -inf,          0x0p+0, INEXACT|OVERFLOW)
T(RN,                     nan,                     nan,          0x0p+0, 0)
T(RU,                     inf,                     inf,          0x0p+0, 0)
T(RU,                    -inf,                    -inf,          0x0p+0, 0)
T(RU,               0x1.8p-26,   0x1.8000000000001p-26,        0x1.cp-2, INEXACT)
T(RU,              -0x1p-1022,              -0x1p-1022,          0x0p+0, INEXACT)
T(RU,-0x1.0000000000001p-1022,-0x1.0000000000001p-1022,          0x0p+0, INEXACT)
T(RU,-0x1.0000000000002p-1022,-0x1.0000000000002p-1022,          0x0p+0, INEXACT)
T(RU,              -0x1p-1021,              -0x1p-1021,          0x0p+0, INEXACT)
T(RU,              -0x1p-1020,              -0x1p-1020,          0x0p+0, INEXACT)
T(RU,              -0x1p-1074,              -0x1p-1074,          0x0p+0, INEXACT|UNDERFLOW)
T(RU,              -0x1p-1073,              -0x1p-1073,          0x0p+0, INEXACT|UNDERFLOW)
T(RU,              -0x1p-1024,              -0x1p-1024,          0x0p+0, INEXACT|UNDERFLOW)
T(RU,              -0x1p-1023,              -0x1p-1023,          0x0p+0, INEXACT|UNDERFLOW)
T(RU,-0x1.ffffffffffffcp-1023,-0x1.ffffffffffffcp-1023,          0x0p+0, INEXACT|UNDERFLOW)
T(RU,-0x1.ffffffffffffep-1023,-0x1.ffffffffffffep-1023,          0x0p+0, INEXACT|UNDERFLOW)
T(RU,              0x1.634p+9,                     inf,          0x0p+0, INEXACT|OVERFLOW)
T(RU,               0x1p+1022,                     inf,          0x0p+0, INEXACT|OVERFLOW)
T(RU,               0x1p+1023,                     inf,          0x0p+0, INEXACT|OVERFLOW)
T(RU, 0x1.ffffffffffffep+1023,                     inf,          0x0p+0, INEXACT|OVERFLOW)
T(RU, 0x1.fffffffffffffp+1023,                     inf,          0x0p+0, INEXACT|OVERFLOW)
T(RU,             -0x1.634p+9,-0x1.fffffffffffffp+1023,          0x1p+0, INEXACT|OVERFLOW)
T(RU,              -0x1p+1022,-0x1.fffffffffffffp+1023,          0x1p+0, INEXACT|OVERFLOW)
T(RU,              -0x1p+1023,-0x1.fffffffffffffp+1023,          0x1p+0, INEXACT|OVERFLOW)
T(RU,-0x1.ffffffffffffep+1023,-0x1.fffffffffffffp+1023,          0x1p+0, INEXACT|OVERFLOW)
T(RU,-0x1.fffffffffffffp+1023,-0x1.fffffffffffffp+1023,          0x1p+0, INEXACT|OVERFLOW)
T(RU,                     nan,                     nan,          0x0p+0, 0)
T(RU,                     nan,                     nan,          0x0p+0, 0)
T(RU,               0x1p-1022, 0x1.0000000000001p-1022,          0x1p+0, INEXACT)
T(RU, 0x1.0000000000001p-1022, 0x1.0000000000002p-1022,          0x1p+0, INEXACT)
T(RU, 0x1.0000000000002p-1022, 0x1.0000000000003p-1022,          0x1p+0, INEXACT)
T(RU,               0x1p-1021, 0x1.0000000000001p-1021,          0x1p+0, INEXACT)
T(RU,               0x1p-1020, 0x1.0000000000001p-1020,          0x1p+0, INEXACT)
T(RU,                 0x1p-26,   0x1.0000000000001p-26,   0x1.aaaaaap-1, INEXACT)
T(RU,                 0x1p-25,   0x1.0000000000001p-25,   0x1.555556p-2, INEXACT)
T(RU,               0x1.4p-25,   0x1.4000000000002p-25,   0x1.655556p-1, INEXACT)
T(RU,                -0x1p-26,                -0x1p-26,   0x1.555556p-3, INEXACT)
T(RU,              -0x1.8p-26,              -0x1.8p-26,        0x1.2p-1, INEXACT)
T(RU,                -0x1p-25,                -0x1p-25,   0x1.555556p-1, INEXACT)
T(RU,              -0x1.4p-25,  -0x1.4000000000001p-25,   0x1.355556p-2, INEXACT)
T(RU,               0x1p-1074,               0x1p-1073,          0x1p+0, INEXACT|UNDERFLOW)
T(RU,               0x1p-1073,             0x1.8p-1073,          0x1p+0, INEXACT|UNDERFLOW)
T(RU,               0x1p-1024, 0x1.0000000000004p-1024,          0x1p+0, INEXACT|UNDERFLOW)
T(RU,               0x1p-1023, 0x1.0000000000002p-1023,          0x1p+0, INEXACT|UNDERFLOW)
T(RU, 0x1.ffffffffffffcp-1023, 0x1.ffffffffffffep-1023,          0x1p+0, INEXACT|UNDERFLOW)
T(RU, 0x1.ffffffffffffep-1023,               0x1p-1022,          0x1p+0, INEXACT|UNDERFLOW)
T(RZ,                     inf,                     inf,          0x0p+0, 0)
T(RZ,                    -inf,                    -inf,          0x0p+0, 0)
T(RZ,               0x1p-1022,               0x1p-1022,          0x0p+0, INEXACT)
T(RZ, 0x1.0000000000001p-1022, 0x1.0000000000001p-1022,          0x0p+0, INEXACT)
T(RZ, 0x1.0000000000002p-1022, 0x1.0000000000002p-1022,          0x0p+0, INEXACT)
T(RZ,               0x1p-1021,               0x1p-1021,          0x0p+0, INEXACT)
T(RZ,               0x1p-1020,               0x1p-1020,          0x0p+0, INEXACT)
T(RZ,                 0x1p-26,                 0x1p-26,  -0x1.555556p-3, INEXACT)
T(RZ,               0x1.8p-26,               0x1.8p-26,       -0x1.2p-1, INEXACT)
T(RZ,              -0x1p-1022,              -0x1p-1022,          0x0p+0, INEXACT)
T(RZ,-0x1.0000000000001p-1022,-0x1.0000000000001p-1022,          0x0p+0, INEXACT)
T(RZ,-0x1.0000000000002p-1022,-0x1.0000000000002p-1022,          0x0p+0, INEXACT)
T(RZ,              -0x1p-1021,              -0x1p-1021,          0x0p+0, INEXACT)
T(RZ,              -0x1p-1020,              -0x1p-1020,          0x0p+0, INEXACT)
T(RZ,               0x1p-1074,               0x1p-1074,          0x0p+0, INEXACT|UNDERFLOW)
T(RZ,               0x1p-1073,               0x1p-1073,          0x0p+0, INEXACT|UNDERFLOW)
T(RZ,               0x1p-1024,               0x1p-1024,          0x0p+0, INEXACT|UNDERFLOW)
T(RZ,               0x1p-1023,               0x1p-1023,          0x0p+0, INEXACT|UNDERFLOW)
T(RZ, 0x1.ffffffffffffcp-1023, 0x1.ffffffffffffcp-1023,          0x0p+0, INEXACT|UNDERFLOW)
T(RZ, 0x1.ffffffffffffep-1023, 0x1.ffffffffffffep-1023,          0x0p+0, INEXACT|UNDERFLOW)
T(RZ,              -0x1p-1074,              -0x1p-1074,          0x0p+0, INEXACT|UNDERFLOW)
T(RZ,              -0x1p-1073,              -0x1p-1073,          0x0p+0, INEXACT|UNDERFLOW)
T(RZ,              -0x1p-1024,              -0x1p-1024,          0x0p+0, INEXACT|UNDERFLOW)
T(RZ,              -0x1p-1023,              -0x1p-1023,          0x0p+0, INEXACT|UNDERFLOW)
T(RZ,-0x1.ffffffffffffcp-1023,-0x1.ffffffffffffcp-1023,          0x0p+0, INEXACT|UNDERFLOW)
T(RZ,-0x1.ffffffffffffep-1023,-0x1.ffffffffffffep-1023,          0x0p+0, INEXACT|UNDERFLOW)
T(RZ,              0x1.634p+9, 0x1.fffffffffffffp+1023,         -0x1p+0, INEXACT|OVERFLOW)
T(RZ,               0x1p+1022, 0x1.fffffffffffffp+1023,         -0x1p+0, INEXACT|OVERFLOW)
T(RZ,               0x1p+1023, 0x1.fffffffffffffp+1023,         -0x1p+0, INEXACT|OVERFLOW)
T(RZ, 0x1.ffffffffffffep+1023, 0x1.fffffffffffffp+1023,         -0x1p+0, INEXACT|OVERFLOW)
T(RZ, 0x1.fffffffffffffp+1023, 0x1.fffffffffffffp+1023,         -0x1p+0, INEXACT|OVERFLOW)
T(RZ,             -0x1.634p+9,-0x1.fffffffffffffp+1023,          0x1p+0, INEXACT|OVERFLOW)
T(RZ,              -0x1p+1022,-0x1.fffffffffffffp+1023,          0x1p+0, INEXACT|OVERFLOW)
T(RZ,              -0x1p+1023,-0x1.fffffffffffffp+1023,          0x1p+0, INEXACT|OVERFLOW)
T(RZ,-0x1.ffffffffffffep+1023,-0x1.fffffffffffffp+1023,          0x1p+0, INEXACT|OVERFLOW)
T(RZ,-0x1.fffffffffffffp+1023,-0x1.fffffffffffffp+1023,          0x1p+0, INEXACT|OVERFLOW)
T(RZ,                     nan,                     nan,          0x0p+0, 0)
T(RZ,                     nan,                     nan,          0x0p+0, 0)
T(RZ,                 0x1p-25,                 0x1p-25,  -0x1.555556p-1, INEXACT)
T(RZ,               0x1.4p-25,   0x1.4000000000001p-25,  -0x1.355556p-2, INEXACT)
T(RZ,                -0x1p-26,                -0x1p-26,   0x1.555556p-3, INEXACT)
T(RZ,              -0x1.8p-26,              -0x1.8p-26,        0x1.2p-1, INEXACT)
T(RZ,                -0x1p-25,                -0x1p-25,   0x1.555556p-1, INEXACT)
T(RZ,              -0x1.4p-25,  -0x1.4000000000001p-25,   0x1.355556p-2, INEXACT)
