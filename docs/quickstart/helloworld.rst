=============
Hello, World!
=============

Now that you've installed IronPLC, it's time to write your first program.

In most programming languages, "Hello, World" prints text to the screen.
IEC 61131-3 is designed for real-time automation controllers that often do
not have a display, so our "Hello, World" will look a little different. In
this chapter, we write the simplest possible program and use IronPLC to
check it for correctness.

--------------------------------------
Create a Project Directory
--------------------------------------

In a Terminal, create a new folder and open it in Visual Studio Code:

.. code-block:: shell
   :caption: Create Project Directory
   :name: newhelloworld

   mkdir helloworld
   cd helloworld
   code .

--------------------------------------
Write Your First Program
--------------------------------------

In Visual Studio Code:

#. In the main menu, select :menuselection:`File --> New File...`.
#. In the :guilabel:`New File...` dialog, select the :menuselection:`Structured Text File` option.
#. Enter the following code into the :guilabel:`Editor`:

   .. code-block::
      :caption: Hello World
      :name: helloworld

      PROGRAM main
         VAR
            Counter : INT := 0;
         END_VAR

         Counter := Counter + 1;

      END_PROGRAM

#. Save the file with the name :file:`main.st`.

That's it — you have written a valid IEC 61131-3 program. If IronPLC's
VS Code extension is installed, you should see no errors highlighted in
the editor.

--------------------------------------
What This Program Does
--------------------------------------

Let's break it down:

- :code:`PROGRAM main` ... :code:`END_PROGRAM` defines a **program** named
  ``main``. A program is the basic unit of control logic in IEC 61131-3,
  similar to a ``main`` function in other languages.

- :code:`VAR` ... :code:`END_VAR` declares a **variable** named ``Counter``
  of type :code:`INT` (a 16-bit signed integer), initialized to 0.

- ``Counter := Counter + 1;`` is an **assignment statement**. The ``:=``
  operator assigns the value on the right to the variable on the left.

This program increments a counter by one each time it runs. On a real PLC,
this would happen on every scan cycle — but we have not configured that yet.
We will add that in :doc:`configuring`.

.. tip::

   For a deeper look at Structured Text syntax, see
   :doc:`/explanation/structured-text-basics`.

--------------------------------------
Next Steps
--------------------------------------

In the next chapter, we will make the program more interesting by connecting
it to inputs and outputs using a doorbell example.

Continue to :doc:`sense-control-actuate`.
