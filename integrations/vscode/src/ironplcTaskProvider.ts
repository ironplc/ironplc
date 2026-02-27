import * as vscode from 'vscode';
import { buildCompileArgs, outputFileNameForFolder } from './taskProviderLogic';

interface IronplcTaskDefinition extends vscode.TaskDefinition {
  task: string;
}

export class IronplcTaskProvider implements vscode.TaskProvider {
  static readonly type = 'ironplc';

  constructor(private compilerPath: string) {}

  async provideTasks(): Promise<vscode.Task[]> {
    const workspaceFolders = vscode.workspace.workspaceFolders;
    if (!workspaceFolders) {
      return [];
    }

    const tasks: vscode.Task[] = [];
    for (const folder of workspaceFolders) {
      tasks.push(this.createCompileTask(folder));
    }
    return tasks;
  }

  resolveTask(task: vscode.Task): vscode.Task | undefined {
    const definition = task.definition as IronplcTaskDefinition;
    if (definition.task === 'compile') {
      const folder = task.scope as vscode.WorkspaceFolder;
      if (folder && folder.uri) {
        return this.createCompileTask(folder);
      }
    }
    return undefined;
  }

  private createCompileTask(folder: vscode.WorkspaceFolder): vscode.Task {
    const definition: IronplcTaskDefinition = {
      type: IronplcTaskProvider.type,
      task: 'compile',
    };

    const outputFileName = outputFileNameForFolder(folder.name);
    const details = buildCompileArgs(folder.uri.fsPath, outputFileName);

    const execution = new vscode.ShellExecution(
      this.compilerPath,
      details.args,
      { cwd: details.cwd },
    );

    const task = new vscode.Task(
      definition,
      folder,
      'compile',
      'ironplc',
      execution,
    );
    task.group = vscode.TaskGroup.Build;
    task.presentationOptions = {
      reveal: vscode.TaskRevealKind.Always,
      panel: vscode.TaskPanelKind.Shared,
    };

    return task;
  }
}
