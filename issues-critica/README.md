# issues-critica/ — critiques waiting for their review

A critique of the project goes here: of the Constitution and the documents, of the code, or of how the code keeps to the Constitution. An agent then:

1. checks every claim against the files, including whoever calls the code a claim names;
2. makes tasks for what holds, in the tracks that own it:
   - its own track's tasks;
   - a request in `issues/requests-<TRK>.md` for a track with an owner;
   - a decision in [issues-human](../issues-human/README.md) where only the maintainer may change the text, such as the Constitution, the RFCs or the roadmap's scope;
3. appends a review to the critique, one row a claim: its verdict (true, partly true, false), what the files show, and where it went;
4. moves it with `git mv` to [issues-critica-done/](../issues-critica-done/README.md).

A claim that the files refute is recorded as false, with the evidence, and makes no task. A vulnerability goes to SECURITY.md's private channel, not into a review.
