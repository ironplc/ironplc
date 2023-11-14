'''
Defines a directive that formats problem codes.
'''
import csv
from sphinx.locale import _
from docutils import nodes
from docutils.parsers.rst import Directive
from sys import exit
from os import listdir
from os.path import join

# Check that for each problem code that the compiler can emit that we have a file
# for that code and create a variable with the message text.
compiler_help_topics = set([v.split('.')[0] for v in listdir(join('compiler', 'problems'))])
extension_help_topics = set([v.split('.')[0] for v in listdir(join('vscode', 'problems'))])
help_topics = compiler_help_topics.union(extension_help_topics)
problem_infos = dict()

class ProblemCode:
    def __init__(self, message):
        self.message = message

definitions = [
    join('..', 'compiler', 'problems', 'resources', 'problem-codes.csv'),
    join('..', 'integrations', 'vscode', 'resources', 'problem-codes.csv')
]

for definition in definitions:
    with open(definition) as fp:
        problem_codes = csv.reader(fp)
        # Skip the header
        next(problem_codes)

        for row in problem_codes:
            code = row[0]
            if code not in help_topics:
                print('Missing help topic for ' + code)
                exit(1)
            problem_infos[code] = ProblemCode(row[2])

class ProblemSummary(Directive):
    required_arguments = 1
    def run(self):
        code = self.arguments[0]
        message = problem_infos[code].message

        items = [
            ProblemSummary.make_item('Code', code),
            ProblemSummary.make_item('Message', message),
        ]
        
        return [nodes.definition_list('', *items)]
    
    @staticmethod
    def make_item(key, value):
        return nodes.definition_list_item(
            '',
            nodes.term('', '', nodes.strong(text=key)),
            nodes.definition('', nodes.paragraph(text=value))
            )

def setup(app):
    app.add_directive("problem-summary", ProblemSummary)

    return {
        'version': '0.1',
        'parallel_read_safe': True,
        'parallel_write_safe': True,
    }