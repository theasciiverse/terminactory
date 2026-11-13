# Guide: belts, backpressure, and why your line stops
board: Guides & Sticky   date: Y129-04-03   author: six_belts   status: open

- six_belts | Y129-04-03 | This is the guide I wish I had been given. It is about one thing: a belt is a queue, and a queue that nobody empties stops.
  **One. A belt is not a pipe.**
  A belt holds what it is given. It takes items from the back and the sides and sends them out of the front. If the front has nowhere to put them, the items stay on the belt. That is not a fault. That is a belt doing its job with nowhere to go.
  **Two. The first symptom is always upstream.**
  When a line stops, the machine you are looking at is almost never the one that stopped first. Walk forward along the belt until you find the machine that is actually stopped, and then ask why. Nine times in ten it is full at the front, blocked at the back, out of coal, or out of EC.
  **Three. Machines say why.**
  The block on an idle machine tells you. FULL means its product has nowhere to go. COAL means it wants fuel. PWR means no EC. WATER means it wants water. Read the block before you take anything apart.
  **Four. A crate is a day of breathing room.**
  A crate between a fast machine and a slow one soaks up the difference. It does not solve the difference. If the slow machine is slower all day, the crate fills all day. Use it while you build the second slow machine.
  **Five. Splitters share, filters choose.**
  A basic splitter hands items out in turn to every belt leading away from it. It has no front. A filter splitter sends one chosen item out its front and everything else out its sides. With no filter set, a filter splitter behaves like a plain one, which is how people lose an afternoon.
  **Six. Cross without cutting.**
  A belt tunnel dives under a busy spot: entrance, then exit further along, anything in between. A long arm reaches two tiles and hops a belt or a machine. Both are cheaper than rebuilding a line to make room.
  - OldHollowmere | Y129-04-03 | Six belts, six faults, six fixes. The man has done it again.
  - TamsinR | Y129-04-04 | I am an apprentice and I have read this three times. The bit about the first symptom being upstream is the bit I needed.
    - six_belts | Y129-04-04 | Read it a fourth time. Then walk the line.

- ivo_p | Y133-08-19 | I would add one thing to point three. Belts themselves need a little EC, and a belt without it shows red and stops moving but does not make a sound or a message. If a whole run is red, check the pole before you check the machine.
  - six_belts | Y133-08-19 | Correct. I will add it to the top post when I next have the edit key.
    - ivo_p | Y133-08-20 | You have the edit key. You are the one who wrote it.
      - six_belts | Y133-08-20 | Then I will add it.

- halvard_k | Y136-02-11 | Hello. Does a fast belt help if the machine in front is slow? I have put one in and the line is no faster.
  - six_belts | Y136-02-11 | No. A fast belt feeding a slow machine just backs up further and faster. Match the belt to the machine, not to the drill.
    - halvard_k | Y136-02-12 | So where do I put it?
      - six_belts | Y136-02-12 | Where the belt itself is the limit. A large drill fills a belt on its own, and that is where a fast belt earns its price.
      - throughput_tony | Y136-02-12 | And upgrade only the stretch that chokes. The upgrade planner will do a whole area at once, which is convenient and expensive. Use it on the choke, not the estate.
        - six_belts | Y136-02-13 | Numbers man agrees with me. I will take it.

- [Deleted User #4471] | Y137-06-14 | One more. If you are sorting a mixed belt, put the filter splitters in series, not side by side. First splitter takes copper out, second takes iron, third takes coal. Side by side they all see the same belt and all take the first thing that arrives.
  - six_belts | Y137-06-14 | That is right and it is worth the paragraph. Series, not parallel.
  - tilda_m | Y137-06-15 | I have seen a man build four of them side by side and call the belt unsortable.

- grey_belt | Y138-05-03 | What is backpressure? People say it and I nod.
  - six_belts | Y138-05-03 | Backpressure is the belt being full. The machine behind it cannot push its product out, so it stops, and the machine behind that stops, and so on back to the drill. The whole line is fine; the end of it is blocked. Clear the end and the whole line starts again in the same order.
  - brass_tally | Y138-05-04 | That is the clearest explanation of it I have read, and I have written one myself.

- crated_up | Y140-07-22 | A warning about crates, since this guide recommends them. Items inside a crate are lost if you remove it. Empty it first. A full crate removed in a hurry is a day of coal on the floor.
  - browncoat_bex | Y140-07-22 | And the machine will not stop you. It will ask about expensive removals, but it will not ask about a crate full of plate.
    - crated_up | Y140-07-23 | It asked me once. I said yes. I regret it.

- second_floor_ivy | Y142-03-16 | Elevators belong in this guide. A car leaves only when it is full, so a trickle of items will sit in it and look like a fault. Feed it in batches and it runs. And plan its spot before you fill the floor, because it needs room on every floor it serves.
  - MillaOnNights | Y142-03-16 | The empty car has frightened every night operator in this plant at least once.
  - second_floor_ivy | Y142-03-17 | Full car or no car. It is on a sign by the lift now.

- mateo_belts | Y147-09-08 | A small addition from a belt fitter. Belts take from their sides as well as their back, which means two lines can feed one belt without a splitter. It also means a belt can be fed by accident by the line running alongside it. If you cannot work out where an item is coming from, look sideways.
  - six_belts | Y147-09-08 | Good. Sideways is the one people forget.
  - pipefitter_uri | Y147-09-09 | Pipes are the same. A machine beside a pipe joins it whether you meant it or not. It is not only belts.

- hem_of_the_line | Y151-05-20 | I have read this guide four times and I still do not understand splitters.
  - unit_ursula | Y151-05-20 | A splitter is a crossroads with no signs. It gives each road a turn. If one road is blocked, the items go down the others. That is all it does.
    - hem_of_the_line | Y151-05-21 | Oh. That is much easier.
      - six_belts | Y151-05-21 | A crossroads with no signs. I am stealing that.
      - ratio_warden | Y151-05-22 | Please do not. It is imprecise. But it is not wrong.

- six_belts | Y157-01-30 | I have been asked to add a line about the upgrade planner, so here it is. The planner swaps belts, drills, panels and assemblers in an area for their next version and you pay the difference. It does not know which stretch was choking. You do. Mark the area you meant, not the area you have.
  - ivo_p | Y157-01-30 | And undo with z if you get it wrong. It is the only thing that has ever saved me from myself.

- brass_tally | Y159-08-11 | I have read this guide many times over the years and it has never needed a correction of substance. That is rare enough that I want it written down. If you are new, read it twice, then walk your line with it in your head. It comes out right in the end.
  - six_belts | Y159-08-11 | Six belts. Six faults. Six fixes.
