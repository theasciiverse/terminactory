# Pipe length does not affect flow, and here is why that matters
board: Fluids   date: Y144-11-18   author: table_herr   status: open

- table_herr | Y144-11-18 | A short note, because I have seen four threads this year asking whether a long pipe is slower. It is not. A joined run of pipe is one pool. It holds fluid and shares it with everything connected. There is no direction and no pressure, so there is no distance to cross. A pipe one tile long and a pipe forty tiles long behave the same, if both hold the same fluid.
  What does matter:
  - One fluid per run. A second fluid will not enter and the first machine that wants it will stop.
  - The pump. Water comes from a pump, and one pump feeds a good number of machines. An advanced pump gives four times the water.
  - The machines on the run. A boiler takes water from any side but its front. A mixer takes it from any side but its front. A pump adds to any pipe it touches.
  - Losing track. A long pipe is easy to lose track of, and you find out what is in it at the worst possible moment.
  - ratio_warden | Y144-11-18 | Accepted. "There is no distance to cross" is the sentence I will quote.
    - pipefitter_uri | Y144-11-19 | It is correct and it is the hardest thing to explain to a new fitter. Water in a pipe is not a river. It is a bucket with a lid.
      - table_herr | Y144-11-19 | It is a bucket with a lid, and the bucket is as big as the run. I wish I had written that.
  - ivo_p | Y144-11-20 | A practical note. Because the run is one pool, a machine beside the pipe joins it whether you meant it to or not. You can water a machine by accident by running a pipe past it. That is usually convenient and occasionally a fault.
    - table_herr | Y144-11-20 | And the same is true of molten metal, which is less convenient.
      - pipefitter_uri | Y144-11-20 | Much less convenient.
  - kettle_ron | Y145-01-11 | Then why does my boiler at the end of a long run starve when I add a mixer at the start?
    - table_herr | Y145-01-11 | Because the pump is not making enough water for both, not because the water is tired. Add a pump or an advanced pump. The distance is not the problem; the supply is.
      - kettle_ron | Y145-01-11 | I have added a pump and it is fine. I feel foolish.
        - table_herr | Y145-01-11 | You are the fifth person this year. It is a good question badly answered by intuition.
  - unit_ursula | Y146-02-19 | A teaching note. I have used the bucket analogy in a class and it worked. The follow-up question was "what happens when the bucket is full?" and the answer is that the machine making the fluid stops. Is that right?
    - table_herr | Y146-02-19 | That is right. A furnace with a full pipe in front of it stops, exactly as it would with a full belt. The pipe is a container, not a drain.
      - unit_ursula | Y146-02-19 | Then I will teach it that way.
  - clean_melt | Y152-03-08 | Seven years later, and this is still the thread I send people to. The bucket with a lid.
    - table_herr | Y152-03-08 | Good. It was the best thing I wrote.
