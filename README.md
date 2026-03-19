The goal of this is to make a version of ALE that can run on 1M+ (aggregate) SPS on a single 4090.

I am going to start with getting the CPU working in a way that transfers well to the CUDA code that I will have to write later.

All credit to https://github.com/Klaus2m5/6502_65C02_functional_tests for the tests. They are essential to having any idea of what is going on.

~~Note: Cycle count is sometimes wrong. As I am not implmenting a TIA, I will not be fixing that.~~

Turns out it is really easy to fix. There is a difference between pragmatism and laziness.