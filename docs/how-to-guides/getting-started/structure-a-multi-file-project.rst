================================
Structure a Multi-File Project
================================

This guide gives practical advice on organizing an IEC 61131-3 project
across multiple files. IronPLC does not enforce any particular layout — use
whatever makes sense for your project.

.. include:: ../../includes/requires-compiler.rst

--------------------------------------
A Recommended Starting Layout
--------------------------------------

For small to medium projects, a flat structure works well:

.. code-block:: text

   my-project/
   ├── config.st        # Configuration, resource, and task definitions
   ├── main.st          # Main program
   ├── utilities.st     # Reusable functions and function blocks
   └── globals.st       # Global variable lists (if needed)

--------------------------------------
Separating Configuration from Logic
--------------------------------------

Keep your :code:`CONFIGURATION` block in its own file. This makes it easy to
create different configurations for testing and production:

.. code-block:: text

   my-project/
   ├── config-production.st    # Production configuration (100ms cycle)
   ├── config-test.st          # Test configuration (different timing)
   ├── main.st
   └── utilities.st

When checking with IronPLC, pass the appropriate configuration:

.. code-block:: shell

   ironplcc check main.st utilities.st config-production.st

--------------------------------------
Checking a Directory
--------------------------------------

If all your :file:`.st` files are in one directory, you can point IronPLC
at the directory instead of listing every file:

.. code-block:: shell

   ironplcc check my-project/

IronPLC finds all :file:`.st` files in the directory and checks them
together.

--------------------------------------
One POU per File
--------------------------------------

As your project grows, consider putting each program, function, or function
block in its own file named after the POU:

.. code-block:: text

   my-project/
   ├── config.st
   ├── main.st
   ├── motor_control.st        # FUNCTION_BLOCK MotorControl
   ├── temperature_monitor.st  # FUNCTION_BLOCK TemperatureMonitor
   └── clamp.st                # FUNCTION Clamp
