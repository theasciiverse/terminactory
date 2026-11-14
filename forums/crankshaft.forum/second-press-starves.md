# Second press starves and the first one is idle
board: Troubleshooting   date: Y144-11-02   author: apprentice_nell   status: solved

- apprentice_nell | Y144-11-02 | I have two presses. The first makes plates and the second makes wire from them. The second one starves and the first one sits idle, and I cannot see how both are true.
  - brass_tally | Y144-11-02 | A plate press makes one plate every two and a half seconds. A wire press eats one plate every five and makes two wire. So the wire press needs half of what the plate press makes, and the other half has to go somewhere.
    - apprentice_nell | Y144-11-02 | Nowhere.
      - brass_tally | Y144-11-02 | Then the plate press fills, stops, and starts again, and the wire press starves in the gaps. You have a line that is exactly fed and a belt with no room in it.
        - apprentice_nell | Y144-11-03 | So put a crate between them.
          - brass_tally | Y144-11-03 | A crate between them, and a trade post for the plates the wire press does not want. Then neither machine is waiting on the other.
            - apprentice_nell | Y144-11-03 | The crate is in and the post takes the spare. Both are running. Edit: solved.
  - crated_up | Y144-11-04 | That is the third time this month I have sold a crate for a line that was exactly fed. It is my best month since Y141.
