==================
Your First Program
==================

Now it's time to write and run your first IEC 61131-3 program. By the end
of this chapter, you will have a working doorbell program running in the
IronPLC virtual machine.

--------------------------------------
Create a Project Directory
--------------------------------------

Open a terminal and create a new folder for your project:

.. code-block:: shell
   :caption: Create Project Directory

   mkdir doorbell
   cd doorbell

Then open the folder in your development environment:

.. code-block:: shell

   code .

.. tip::

   If you are using Cursor, use ``cursor .`` instead of ``code .``.

--------------------------------------
Write the Program
--------------------------------------

In your development environment:

#. In the main menu, select :menuselection:`File --> New File...`.
#. In the :guilabel:`New File...` dialog, select the :menuselection:`Structured Text File` option.
#. Enter the following code into the :guilabel:`Editor`:

   .. code-block::
      :caption: main.st — Doorbell Program
      :name: doorbell-program

      PROGRAM main
         VAR
            Button : BOOL;
            Buzzer : BOOL;
         END_VAR

         Buzzer := NOT Button;

      END_PROGRAM

#. Save the file with the name :file:`main.st`.

If the IronPLC extension is installed, you should see no errors highlighted
in the editor.

--------------------------------------
What This Program Does
--------------------------------------

Let's break it down:

- :code:`PROGRAM main` ... :code:`END_PROGRAM` defines a **program** named
  ``main``. A program is the basic unit of control logic in IEC 61131-3,
  similar to a ``main`` function in other languages.

- :code:`VAR` ... :code:`END_VAR` declares two **variables** of type
  :code:`BOOL`. ``Button`` represents the sensor input and ``Buzzer``
  represents the actuator output.

- ``Buzzer := NOT Button;`` is an **assignment statement**. The ``:=``
  operator assigns the value on the right to the variable on the left.
  When ``Button`` is ``FALSE`` (not pressed), ``Buzzer`` is ``TRUE``
  (sounding).

--------------------------------------
Run the Program
--------------------------------------

Look for the :guilabel:`Run Program` link that appears above the
``PROGRAM main`` line in the editor. This is a code lens provided by the
IronPLC extension.

#. Click :guilabel:`Run Program`.
#. The :guilabel:`IronPLC Run` output panel opens automatically. It shows
   the current scan cycle number and the value of every variable, updating
   as the program runs. You should see output like:

   .. code-block:: text

      Scan cycle: 1
      ---
        Button : BOOL = FALSE
        Buzzer : BOOL = TRUE

#. Click :guilabel:`Stop` above the ``PROGRAM`` line to end execution.

``Button`` starts as ``FALSE`` (the default for :code:`BOOL`), so
``NOT Button`` evaluates to ``TRUE``, and the buzzer sounds. This is
exactly the sense-control-actuate cycle in action — even though there is
no physical hardware connected yet.

.. tip::

   You can also check, compile, and run from the command line. See
   :doc:`/how-to-guides/getting-started/check-compile-run-from-cli`.

--------------------------------------
Next Steps
--------------------------------------

You now have a working program. In the next chapter, you will add a timer
to make the buzzer pulse automatically and learn how the configuration
block controls the scheduling.

Continue to :doc:`configuring`.
